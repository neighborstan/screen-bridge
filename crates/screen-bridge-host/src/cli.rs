use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "screen-bridge-host")]
#[command(version)]
#[command(about = "ScreenBridge RTSP host")]
pub(crate) struct Cli {
    #[arg(long, value_name = "PATH", help = "Path to host TOML config")]
    pub(crate) config: Option<PathBuf>,

    #[arg(
        long,
        conflicts_with = "print_vlc_url",
        help = "Run host diagnostics and exit"
    )]
    pub(crate) diagnose: bool,

    #[arg(long, help = "Print the full VLC RTSP URL with token and exit")]
    pub(crate) print_vlc_url: bool,
}

impl Cli {
    pub(crate) fn required_config_path(&self) -> Result<&Path> {
        self.config
            .as_deref()
            .ok_or_else(|| anyhow!("укажите --config <PATH> или запустите --diagnose"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_should_parse_config_flag() {
        // Given
        let args = ["screen-bridge-host", "--config", "config/host.toml"];

        // When
        let cli = Cli::try_parse_from(args).unwrap();

        // Then
        assert_eq!(cli.config.unwrap(), PathBuf::from("config/host.toml"));
        assert!(!cli.diagnose);
        assert!(!cli.print_vlc_url);
    }

    #[test]
    fn cli_should_parse_diagnose_without_config() {
        // Given
        let args = ["screen-bridge-host", "--diagnose"];

        // When
        let cli = Cli::try_parse_from(args).unwrap();

        // Then
        assert!(cli.config.is_none());
        assert!(cli.diagnose);
    }

    #[test]
    fn cli_should_reject_diagnose_with_print_vlc_url() {
        // Given
        let args = ["screen-bridge-host", "--diagnose", "--print-vlc-url"];

        // When
        let result = Cli::try_parse_from(args);

        // Then
        assert!(result.is_err());
    }
}
