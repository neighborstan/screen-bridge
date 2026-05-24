use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "screen-bridge-viewer")]
#[command(version)]
#[command(about = "ScreenBridge RTSP viewer")]
pub(crate) struct Cli {
    #[arg(long, value_name = "PATH", help = "Path to viewer TOML config")]
    pub(crate) config: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_should_parse_config_flag() {
        // Given
        let args = ["screen-bridge-viewer", "--config", "config/viewer.toml"];

        // When
        let cli = Cli::try_parse_from(args).unwrap();

        // Then
        assert_eq!(cli.config, PathBuf::from("config/viewer.toml"));
    }

    #[test]
    fn cli_should_require_config_flag() {
        // Given
        let args = ["screen-bridge-viewer"];

        // When
        let result = Cli::try_parse_from(args);

        // Then
        assert!(result.is_err());
    }
}
