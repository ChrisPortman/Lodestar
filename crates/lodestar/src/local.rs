//! Local mode (`--local`): a single-user Lodestar on loopback, its files in
//! the per-user data directory, at most one instance per user.

use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// How long a second launch waits for the running instance to record its port.
const PORT_WAIT: Duration = Duration::from_secs(5);

/// The per-user data directory, `<OS data dir>/lodestar`, created if missing
/// and kept private to the user (mode 0700) on Unix.
pub fn data_dir() -> io::Result<PathBuf> {
    let dir = dirs::data_dir()
        .ok_or_else(|| io::Error::other("no per-user data directory on this system"))?
        .join("lodestar");
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(&dir)?;
    #[cfg(unix)]
    std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    Ok(dir)
}

/// The outcome of claiming the data directory.
pub enum Claim {
    /// This process is the instance. Hold the lock until it exits.
    Acquired(Instance),
    /// Another instance is running, on this port.
    Running(u16),
}

/// The lock held by the running instance. The OS releases it when the process
/// exits, however it exits, so a stale lock never blocks a later launch. The
/// port file is left behind; the next instance to claim the lock clears it.
pub struct Instance {
    _lock: File,
    port_file: PathBuf,
}

impl Instance {
    /// Records the bound port for later launches to find. Written then renamed
    /// into place, so a reader never sees a partial port.
    pub fn record_port(&self, port: u16) -> io::Result<()> {
        let tmp = self.port_file.with_extension("port.tmp");
        std::fs::write(&tmp, port.to_string())?;
        std::fs::rename(tmp, &self.port_file)
    }
}

/// Takes the instance lock in `dir`, or finds the port of the instance that
/// holds it. The port lives in its own file because an exclusive lock blocks
/// reads on Windows.
pub fn claim(dir: &Path) -> io::Result<Claim> {
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("lodestar.lock"))?;
    let port_file = dir.join("lodestar.port");
    match lock.try_lock() {
        Ok(()) => {
            match std::fs::remove_file(&port_file) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
            Ok(Claim::Acquired(Instance {
                _lock: lock,
                port_file,
            }))
        }
        Err(std::fs::TryLockError::WouldBlock) => running_port(&port_file).map(Claim::Running),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

/// Reads the running instance's port, waiting briefly in case it has taken
/// the lock but not yet bound. Blocking is fine: nothing else runs yet.
fn running_port(port_file: &Path) -> io::Result<u16> {
    let started = Instant::now();
    loop {
        if let Some(port) = std::fs::read_to_string(port_file)
            .ok()
            .and_then(|s| s.trim().parse().ok())
        {
            return Ok(port);
        }
        if started.elapsed() > PORT_WAIT {
            return Err(io::Error::other(format!(
                "another instance holds the lock but {} has no port",
                port_file.display()
            )));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Prints the URL of the instance on `port` to stdout, then opens it in the
/// default browser, warning if that fails. The URL names 127.0.0.1, where the
/// server is bound, since `localhost` may resolve to `::1` first.
pub fn open(port: u16) {
    let url = format!("http://127.0.0.1:{port}");
    println!("Lodestar is running at {url}");
    if let Err(error) = webbrowser::open(&url) {
        tracing::warn!(%error, %url, "could not open a default browser");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_claim_finds_the_first_instances_port() {
        let dir = tempfile::tempdir().unwrap();
        let Claim::Acquired(instance) = claim(dir.path()).unwrap() else {
            panic!("first claim should acquire the lock");
        };
        instance.record_port(4321).unwrap();
        assert!(matches!(claim(dir.path()).unwrap(), Claim::Running(4321)));

        drop(instance);
        assert!(matches!(claim(dir.path()).unwrap(), Claim::Acquired(_)));
    }

    #[test]
    fn acquiring_clears_a_stale_port() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("lodestar.port"), "1").unwrap();
        assert!(matches!(claim(dir.path()).unwrap(), Claim::Acquired(_)));
        assert!(!dir.path().join("lodestar.port").exists());
    }
}
