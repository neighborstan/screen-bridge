use anyhow::{Context, Result};
use clap::Parser;
use screen_bridge_core::config::load_viewer;
use screen_bridge_core::logging;

mod cli;

use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_viewer(&cli.config).with_context(|| {
        format!(
            "не удалось загрузить viewer config `{}`",
            cli.config.display()
        )
    })?;

    let _logging_guard = logging::init(&config.logging).context("не удалось настроить logging")?;

    screen_bridge_viewer::run(config)
}
