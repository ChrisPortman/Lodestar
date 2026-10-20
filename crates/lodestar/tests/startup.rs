use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn lodestar(dir: &tempfile::TempDir) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lodestar"));
    cmd.current_dir(dir.path())
        .env_remove("LODESTAR_CONFIG")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn get_healthz(port: u16) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream
        .write_all(b"GET /api/v1/healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    Some(response)
}

#[test]
fn starts_and_serves_healthz_within_two_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let started = Instant::now();
    let mut child = lodestar(&dir)
        .env("LODESTAR_SERVER__PORT", port.to_string())
        .spawn()
        .unwrap();

    let response = loop {
        if let Some(response) = get_healthz(port) {
            break Some(response);
        }
        if started.elapsed() > Duration::from_secs(2) {
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    child.kill().unwrap();
    child.wait().unwrap();

    let response = response.expect("no /api/v1/healthz response within 2s (NFR-42)");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains(r#""database":"reachable""#), "{response}");
    assert!(dir.path().join("lodestar.db").exists());
    assert!(dir.path().join("lodestar-session.key").exists());
}

#[test]
fn invalid_configuration_exits_with_ex_config() {
    let dir = tempfile::tempdir().unwrap();
    let status = lodestar(&dir)
        .env("LODESTAR_SERVER__PORT", "0")
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(78));
}

#[test]
fn unopenable_database_exits_with_ex_config() {
    let dir = tempfile::tempdir().unwrap();
    let status = lodestar(&dir)
        .env("LODESTAR_SERVER__PORT", free_port().to_string())
        .env("LODESTAR_DATABASE__URL", "sqlite://missing-dir/lodestar.db")
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(78));
}

#[test]
fn missing_explicit_config_file_exits_with_ex_config() {
    let dir = tempfile::tempdir().unwrap();
    let status = lodestar(&dir)
        .args(["--config", "missing.toml"])
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(78));
}

#[cfg(unix)]
#[test]
fn sigterm_shuts_down_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let mut child = lodestar(&dir)
        .env("LODESTAR_SERVER__PORT", port.to_string())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while get_healthz(port).is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "server did not start"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let sent = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(sent.success());
    let stopping = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if stopping.elapsed() > Duration::from_secs(5) {
            child.kill().unwrap();
            panic!("still running 5s after SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0), "{status:?}");
    assert!(
        TcpStream::connect(("127.0.0.1", port)).is_err(),
        "no longer listening"
    );
}

/// `lodestar --local` with its data directory and home under `dir`, and a
/// browser that does nothing.
#[cfg(target_os = "linux")]
fn lodestar_local(dir: &tempfile::TempDir) -> Command {
    let mut cmd = lodestar(dir);
    cmd.arg("--local")
        .env("HOME", dir.path())
        .env("XDG_DATA_HOME", dir.path().join("data"))
        .env("BROWSER", "true")
        .env("LODESTAR_SERVER__PORT", "1")
        .stdout(Stdio::piped());
    cmd
}

#[cfg(target_os = "linux")]
fn read_url(stdout: impl Read) -> String {
    use std::io::BufRead;
    std::io::BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
        .find_map(|line| Some(line.strip_prefix("Lodestar is running at ")?.to_owned()))
        .expect("no URL printed")
}

#[cfg(target_os = "linux")]
#[test]
fn local_mode_serves_from_the_data_dir_and_a_second_launch_reuses_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = lodestar_local(&dir).spawn().unwrap();
    let url = read_url(child.stdout.take().unwrap());
    let port: u16 = url.rsplit(':').next().unwrap().parse().unwrap();
    assert_ne!(port, 1, "--local must ignore LODESTAR_SERVER__PORT");
    let healthz = get_healthz(port);

    let second = lodestar_local(&dir).output().unwrap();
    child.kill().unwrap();
    child.wait().unwrap();

    assert!(healthz.unwrap().starts_with("HTTP/1.1 200"));
    let data = dir.path().join("data/lodestar");
    assert!(data.join("lodestar.db").exists());
    assert!(data.join("lodestar-session.key").exists());
    assert!(!dir.path().join("lodestar.db").exists());
    assert_eq!(second.status.code(), Some(0));
    assert_eq!(read_url(second.stdout.as_slice()), url);
}
