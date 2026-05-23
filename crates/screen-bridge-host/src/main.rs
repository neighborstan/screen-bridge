use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use screen_bridge_core::config::load_host;
use screen_bridge_core::logging;

#[derive(Debug, Parser)]
#[command(name = "screen-bridge-host")]
#[command(version)]
#[command(about = "ScreenBridge RTSP host")]
struct Cli {
    #[arg(long, value_name = "PATH")]
    config: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_host(&cli.config).with_context(|| {
        format!(
            "не удалось загрузить host config `{}`",
            cli.config.display()
        )
    })?;
    let _logging_guard = logging::init(&config.logging).context("не удалось настроить logging")?;

    screen_bridge_host::run(config)
}
