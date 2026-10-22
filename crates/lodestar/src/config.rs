//! Configuration: defaults < TOML file < `LODESTAR_*` env (NFR-20), then
//! validation. `Config` stays in this crate; other crates get plain values.

use std::{
    fmt,
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

/// Minimum session key length in bytes (cookie signing + encryption).
pub const SESSION_KEY_MIN_LEN: usize = 32;
/// Length in bytes of a generated session key.
pub const SESSION_KEY_LEN: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub server: Server,
    pub database: Database,
    pub log: Log,
    pub session: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub bind: IpAddr,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Database {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Log {
    pub level: String,
    pub format: LogFormat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Json,
    Pretty,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Session {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub key_file: PathBuf,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("key", &self.key.as_ref().map(|_| "<redacted>"))
            .field("key_file", &self.key_file)
            .finish()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: Server {
                bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 8080,
            },
            database: Database {
                url: "sqlite://lodestar.db".to_owned(),
            },
            log: Log {
                level: "info".to_owned(),
                format: LogFormat::Json,
            },
            session: Session {
                key: None,
                key_file: PathBuf::from("lodestar-session.key"),
            },
        }
    }
}

/// A configuration error, naming the offending key and where its value came
/// from when known.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config file {0} does not exist")]
    MissingFile(PathBuf),
    #[error("invalid value for `{key}` (from {origin}): {message}")]
    Invalid {
        key: String,
        origin: String,
        message: String,
    },
    #[error("session key file {path}: {error}")]
    KeyFile {
        path: PathBuf,
        error: std::io::Error,
    },
}

/// Where the config file comes from: `--config`, else `LODESTAR_CONFIG`,
/// else `./lodestar.toml` if present.
fn config_file(cli: Option<&Path>) -> Result<Option<PathBuf>, ConfigError> {
    let explicit = cli
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("LODESTAR_CONFIG").map(PathBuf::from));
    match explicit {
        Some(path) if path.is_file() => Ok(Some(path)),
        Some(path) => Err(ConfigError::MissingFile(path)),
        None => {
            let default = PathBuf::from("lodestar.toml");
            Ok(default.is_file().then_some(default))
        }
    }
}

/// Loads and validates the configuration.
pub fn load(cli: Option<&Path>) -> Result<Config, ConfigError> {
    let figment = figment(cli, Config::default())?;
    let config = extract(&figment)?;
    validate_with_origin(&config, &figment)?;
    Ok(config)
}

/// Settings that local mode owns: values for them from a file or the
/// environment are ignored.
pub const LOCAL_KEYS: [&str; 5] = [
    "server.bind",
    "server.port",
    "database.url",
    "session.key",
    "session.key_file",
];

/// Loads the configuration for local mode: like [`load`], but logs default to
/// `pretty`, and the [`LOCAL_KEYS`] are forced to loopback, any free port
/// (`server.port` 0) and files in `data_dir`. Also returns the forced keys that
/// a file or the environment tried to set, so the caller can warn about them.
pub fn load_local(
    cli: Option<&Path>,
    data_dir: &Path,
) -> Result<(Config, Vec<&'static str>), ConfigError> {
    let mut defaults = Config::default();
    defaults.log.format = LogFormat::Pretty;
    let figment = figment(cli, defaults)?;
    let mut config = extract(&figment)?;
    // `Serialized` names its source after the serialized type, so a key whose
    // metadata has that name came from the defaults rather than a file or env.
    let default_source = std::any::type_name::<Config>();
    let ignored = LOCAL_KEYS
        .into_iter()
        .filter(|key| {
            figment
                .find_metadata(key)
                .is_some_and(|metadata| metadata.name != default_source)
        })
        .collect();

    config.server = Config::default().server;
    config.database.url = format!("sqlite://{}", data_dir.join("lodestar.db").display());
    config.session = Session {
        key: None,
        key_file: data_dir.join("lodestar-session.key"),
    };
    validate_with_origin(&config, &figment)?;
    config.server.port = 0;
    Ok((config, ignored))
}

fn figment(cli: Option<&Path>, defaults: Config) -> Result<Figment, ConfigError> {
    let mut figment = Figment::from(Serialized::defaults(defaults));
    if let Some(file) = config_file(cli)? {
        figment = figment.merge(Toml::file_exact(file));
    }
    Ok(figment.merge(
        Env::prefixed("LODESTAR_")
            .ignore(&["CONFIG"])
            .split("__")
            .lowercase(true),
    ))
}

fn extract(figment: &Figment) -> Result<Config, ConfigError> {
    figment.extract().map_err(|e| from_figment(&e))
}

fn validate_with_origin(config: &Config, figment: &Figment) -> Result<(), ConfigError> {
    validate(config).map_err(|(key, message)| ConfigError::Invalid {
        origin: figment
            .find_metadata(key)
            .map(describe)
            .unwrap_or_else(|| "unknown".to_owned()),
        key: key.to_owned(),
        message,
    })
}

fn from_figment(error: &figment::Error) -> ConfigError {
    ConfigError::Invalid {
        key: error.path.join("."),
        origin: error
            .metadata
            .as_ref()
            .map(describe)
            .unwrap_or_else(|| "unknown".to_owned()),
        message: error.kind.to_string(),
    }
}

fn describe(metadata: &figment::Metadata) -> String {
    match &metadata.source {
        Some(source) => format!("{} {source}", metadata.name),
        None => metadata.name.to_string(),
    }
}

fn validate(config: &Config) -> Result<(), (&'static str, String)> {
    if config.server.port == 0 {
        return Err(("server.port", "must be between 1 and 65535".to_owned()));
    }
    match config.database.url.split_once("://") {
        Some(("sqlite", _)) => {}
        _ => {
            return Err((
                "database.url",
                format!(
                    "unsupported database URL {:?}; expected sqlite://<path>",
                    config.database.url
                ),
            ));
        }
    }
    if let Err(e) = EnvFilter::try_new(&config.log.level) {
        return Err(("log.level", e.to_string()));
    }
    if let Some(key) = &config.session.key {
        decode_key(key).map_err(|m| ("session.key", m))?;
    }
    Ok(())
}

fn decode_key(encoded: &str) -> Result<Vec<u8>, String> {
    let bytes = BASE64
        .decode(encoded.trim())
        .map_err(|e| format!("not valid base64: {e}"))?;
    if bytes.len() < SESSION_KEY_MIN_LEN {
        return Err(format!(
            "must be at least {SESSION_KEY_MIN_LEN} bytes, got {}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

/// Where the session key came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    Config,
    File(PathBuf),
    Generated(PathBuf),
}

/// Resolves the session key: `session.key`, else the contents of
/// `session.key_file`, else a newly generated key written there with mode 0600.
pub fn session_key(session: &Session) -> Result<(Vec<u8>, KeySource), ConfigError> {
    if let Some(key) = &session.key {
        let bytes = decode_key(key).map_err(|message| ConfigError::Invalid {
            key: "session.key".to_owned(),
            origin: "config".to_owned(),
            message,
        })?;
        return Ok((bytes, KeySource::Config));
    }
    let path = &session.key_file;
    let key_file_error = |error| ConfigError::KeyFile {
        path: path.clone(),
        error,
    };
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let bytes = decode_key(&contents).map_err(|message| ConfigError::Invalid {
                key: "session.key_file".to_owned(),
                origin: path.display().to_string(),
                message,
            })?;
            Ok((bytes, KeySource::File(path.clone())))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut bytes = vec![0u8; SESSION_KEY_LEN];
            getrandom::fill(&mut bytes).map_err(|e| key_file_error(std::io::Error::other(e)))?;
            write_private(path, BASE64.encode(&bytes).as_bytes()).map_err(key_file_error)?;
            Ok((bytes, KeySource::Generated(path.clone())))
        }
        Err(e) => Err(key_file_error(e)),
    }
}

fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

#[cfg(test)]
#[allow(clippy::result_large_err)] // figment::Jail's closure signature
mod tests {
    use super::*;
    use figment::Jail;

    fn load_err(cli: Option<&Path>) -> String {
        load(cli).unwrap_err().to_string()
    }

    #[test]
    fn defaults_apply_without_file_or_env() {
        Jail::expect_with(|_| {
            assert_eq!(load(None).unwrap(), Config::default());
            Ok(())
        });
    }

    #[test]
    fn env_overrides_file_which_overrides_defaults() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "lodestar.toml",
                "[server]\nport = 9000\nbind = \"0.0.0.0\"\n[log]\nformat = \"pretty\"\n",
            )?;
            jail.set_env("LODESTAR_SERVER__PORT", "9100");
            let config = load(None).unwrap();
            assert_eq!(config.server.port, 9100);
            assert_eq!(config.server.bind.to_string(), "0.0.0.0");
            assert_eq!(config.log.format, LogFormat::Pretty);
            assert_eq!(config.log.level, "info");
            Ok(())
        });
    }

    #[test]
    fn local_mode_forces_network_and_files_and_reports_what_it_ignored() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "lodestar.toml",
                "[server]\nport = 0\nbind = \"0.0.0.0\"\n[log]\nlevel = \"debug\"\n",
            )?;
            jail.set_env("LODESTAR_DATABASE__URL", "sqlite://elsewhere.db");
            let data = Path::new("/data/lodestar");
            let (config, ignored) = load_local(None, data).unwrap();
            assert_eq!(config.server.bind, IpAddr::V4(Ipv4Addr::LOCALHOST));
            assert_eq!(config.server.port, 0);
            assert_eq!(
                config.database.url,
                format!("sqlite://{}", data.join("lodestar.db").display())
            );
            assert_eq!(config.session.key_file, data.join("lodestar-session.key"));
            assert_eq!(config.log.level, "debug");
            assert_eq!(config.log.format, LogFormat::Pretty);
            assert_eq!(ignored, ["server.bind", "server.port", "database.url"]);
            Ok(())
        });
    }

    #[test]
    fn local_mode_ignores_nothing_by_default_and_honours_log_format() {
        Jail::expect_with(|jail| {
            jail.set_env("LODESTAR_LOG__FORMAT", "json");
            let (config, ignored) = load_local(None, Path::new("/d")).unwrap();
            assert!(ignored.is_empty(), "{ignored:?}");
            assert_eq!(config.log.format, LogFormat::Json);
            Ok(())
        });
    }

    #[test]
    fn double_underscore_nests_env_keys() {
        Jail::expect_with(|jail| {
            jail.set_env("LODESTAR_DATABASE__URL", "sqlite://other.db");
            jail.set_env("LODESTAR_SESSION__KEY_FILE", "k.key");
            let config = load(None).unwrap();
            assert_eq!(config.database.url, "sqlite://other.db");
            assert_eq!(config.session.key_file, PathBuf::from("k.key"));
            Ok(())
        });
    }

    #[test]
    fn explicit_config_path_is_used_over_default_file() {
        Jail::expect_with(|jail| {
            jail.create_file("lodestar.toml", "[server]\nport = 1111\n")?;
            jail.create_file("custom.toml", "[server]\nport = 2222\n")?;
            assert_eq!(
                load(Some(Path::new("custom.toml"))).unwrap().server.port,
                2222
            );
            jail.set_env("LODESTAR_CONFIG", "custom.toml");
            assert_eq!(load(None).unwrap().server.port, 2222);
            Ok(())
        });
    }

    #[test]
    fn explicitly_named_missing_file_is_an_error() {
        Jail::expect_with(|jail| {
            assert!(load_err(Some(Path::new("nope.toml"))).contains("nope.toml"));
            jail.set_env("LODESTAR_CONFIG", "gone.toml");
            assert!(load_err(None).contains("gone.toml"));
            Ok(())
        });
    }

    #[test]
    fn unknown_keys_are_rejected() {
        Jail::expect_with(|jail| {
            jail.create_file("lodestar.toml", "[server]\nprot = 1\n")?;
            let err = load_err(None);
            assert!(err.contains("prot"), "{err}");
            Ok(())
        });
    }

    #[test]
    fn invalid_values_name_the_key_and_source() {
        Jail::expect_with(|jail| {
            jail.set_env("LODESTAR_SERVER__PORT", "0");
            let err = load_err(None);
            assert!(err.contains("server.port"), "{err}");
            assert!(err.contains("LODESTAR_"), "{err}");
            Ok(())
        });
        Jail::expect_with(|jail| {
            jail.create_file("lodestar.toml", "[database]\nurl = \"postgres://x\"\n")?;
            let err = load_err(None);
            assert!(err.contains("database.url"), "{err}");
            assert!(err.contains("lodestar.toml"), "{err}");
            Ok(())
        });
        Jail::expect_with(|jail| {
            jail.set_env("LODESTAR_LOG__LEVEL", "info,=[");
            assert!(load_err(None).contains("log.level"));
            Ok(())
        });
        Jail::expect_with(|jail| {
            jail.set_env("LODESTAR_LOG__FORMAT", "xml");
            assert!(load_err(None).contains("format"));
            Ok(())
        });
        Jail::expect_with(|jail| {
            jail.set_env("LODESTAR_SESSION__KEY", BASE64.encode([1u8; 16]));
            let err = load_err(None);
            assert!(err.contains("session.key"), "{err}");
            assert!(err.contains("at least 32 bytes"), "{err}");
            Ok(())
        });
        Jail::expect_with(|jail| {
            jail.set_env("LODESTAR_SESSION__KEY", BASE64.encode([1u8; 32]));
            assert!(load(None).is_ok());
            Ok(())
        });
    }

    #[test]
    fn session_key_from_config_is_used() {
        let session = Session {
            key: Some(BASE64.encode([7u8; 64])),
            key_file: PathBuf::from("/nonexistent/never-written"),
        };
        assert_eq!(
            session_key(&session).unwrap(),
            (vec![7u8; 64], KeySource::Config)
        );
    }

    #[test]
    fn missing_key_file_is_generated_with_mode_0600_then_reused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.key");
        let session = Session {
            key: None,
            key_file: path.clone(),
        };

        let (generated, source) = session_key(&session).unwrap();
        assert_eq!(generated.len(), 64);
        assert_eq!(source, KeySource::Generated(path.clone()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        let (reread, source) = session_key(&session).unwrap();
        assert_eq!(reread, generated);
        assert_eq!(source, KeySource::File(path));
    }

    #[test]
    fn short_key_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.key");
        std::fs::write(&path, BASE64.encode([1u8; 10])).unwrap();
        let session = Session {
            key: None,
            key_file: path,
        };
        let err = session_key(&session).unwrap_err().to_string();
        assert!(err.contains("session.key_file"), "{err}");
    }

    #[test]
    fn debug_output_redacts_the_session_key() {
        let mut config = Config::default();
        config.session.key = Some("c2VjcmV0".to_owned());
        let shown = format!("{config:?}");
        assert!(!shown.contains("c2VjcmV0"));
        assert!(shown.contains("<redacted>"));
    }
}
