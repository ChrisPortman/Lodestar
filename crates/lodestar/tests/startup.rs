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
