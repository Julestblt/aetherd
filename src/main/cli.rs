use std::path::PathBuf;

pub(super) enum Cli {
    Run { config: Option<PathBuf> },
    Help,
    Version,
}

impl Cli {
    pub(super) fn from_env() -> Result<Self, String> {
        Self::parse(std::env::args().skip(1))
    }

    fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut args = args.into_iter();
        let mut config = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Ok(Self::Help),
                "-V" | "--version" => return Ok(Self::Version),
                "--config" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--config requires a path".to_owned())?;
                    config = Some(PathBuf::from(value));
                }
                other if other.starts_with("--config=") => {
                    let value = &other["--config=".len()..];
                    if value.is_empty() {
                        return Err("--config requires a path".to_owned());
                    }
                    config = Some(PathBuf::from(value));
                }
                other => return Err(format!("unrecognized argument: {other}")),
            }
        }
        Ok(Self::Run { config })
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        Cli::parse(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn no_arguments_runs_without_config() {
        assert!(matches!(parse(&[]), Ok(Cli::Run { config: None })));
    }

    #[test]
    fn config_flag_accepts_separate_value() {
        assert!(matches!(
            parse(&["--config", "custom.toml"]),
            Ok(Cli::Run { config: Some(_) })
        ));
    }

    #[test]
    fn config_flag_accepts_equals_form() {
        assert!(matches!(
            parse(&["--config=custom.toml"]),
            Ok(Cli::Run { config: Some(_) })
        ));
    }

    #[test]
    fn config_flag_requires_a_value() {
        assert!(parse(&["--config"]).is_err());
        assert!(parse(&["--config="]).is_err());
    }

    #[test]
    fn help_and_version_short_circuit() {
        assert!(matches!(parse(&["--help"]), Ok(Cli::Help)));
        assert!(matches!(parse(&["-V"]), Ok(Cli::Version)));
    }

    #[test]
    fn unknown_argument_is_rejected() {
        assert!(parse(&["--nope"]).is_err());
    }
}
