//! Command line: `--config <path>`, `--help`, `--version`, and `openapi [path]`.

use std::{ffi::OsString, path::PathBuf};

pub const HELP: &str = "\
Usage: lodestar [--config <path>]
       lodestar openapi [<path>]

Commands:
  openapi [<path>]  Write the OpenAPI document (default openapi/openapi.json)

Options:
  --config <path>  Configuration file (or LODESTAR_CONFIG; default ./lodestar.toml if present)
  -h, --help       Print help
  -V, --version    Print version

Settings can also be given as LODESTAR_* environment variables, e.g. LODESTAR_SERVER__PORT.
";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Run { config: Option<PathBuf> },
    OpenApi { out: PathBuf },
    Help,
    Version,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let mut args = args.into_iter().peekable();
    if args.peek().and_then(|a| a.to_str()) == Some("openapi") {
        args.next();
        let out = args
            .next()
            .map_or_else(|| "openapi/openapi.json".into(), PathBuf::from);
        if let Some(extra) = args.next() {
            return Err(format!("unexpected argument {extra:?}"));
        }
        return Ok(Command::OpenApi { out });
    }
    let mut config = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => return Ok(Command::Help),
            Some("-V" | "--version") => return Ok(Command::Version),
            Some("--config") => {
                let path = args.next().ok_or("--config needs a path")?;
                config = Some(PathBuf::from(path));
            }
            Some(s) if s.starts_with("--config=") => {
                config = Some(PathBuf::from(&s["--config=".len()..]));
            }
            _ => return Err(format!("unexpected argument {arg:?}")),
        }
    }
    Ok(Command::Run { config })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn parses_config_help_and_version() {
        assert_eq!(parse_strs(&[]), Ok(Command::Run { config: None }));
        assert_eq!(
            parse_strs(&["--config", "a.toml"]),
            Ok(Command::Run {
                config: Some("a.toml".into())
            })
        );
        assert_eq!(
            parse_strs(&["--config=b.toml"]),
            Ok(Command::Run {
                config: Some("b.toml".into())
            })
        );
        assert_eq!(parse_strs(&["--help"]), Ok(Command::Help));
        assert_eq!(parse_strs(&["-V"]), Ok(Command::Version));
    }

    #[test]
    fn parses_openapi_with_an_optional_output_path() {
        assert_eq!(
            parse_strs(&["openapi"]),
            Ok(Command::OpenApi {
                out: "openapi/openapi.json".into()
            })
        );
        assert_eq!(
            parse_strs(&["openapi", "x.json"]),
            Ok(Command::OpenApi {
                out: "x.json".into()
            })
        );
        assert!(parse_strs(&["openapi", "a", "b"]).is_err());
    }

    #[test]
    fn rejects_other_arguments() {
        assert!(parse_strs(&["--port", "1"]).is_err());
        assert!(parse_strs(&["--config"]).is_err());
    }
}
