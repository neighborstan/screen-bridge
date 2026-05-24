use anyhow::{Context, Result};
use clap::Parser;
use screen_bridge_core::config::load_host;
use screen_bridge_core::logging;

mod cli;

use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.diagnose {
        let report = screen_bridge_host::diagnose(cli.config.as_deref());
        print!("{report}");
        if report.has_failures() {
            std::process::exit(1);
        }

        return Ok(());
    }

    let config_path = cli.required_config_path()?;
    let config = load_host(config_path).with_context(|| {
        format!(
            "не удалось загрузить host config `{}`",
            config_path.display()
        )
    })?;

    if cli.print_vlc_url {
        let bind_ip = screen_bridge_host::resolve_bind_ip(&config)?;
        println!("{}", screen_bridge_host::build_vlc_url(&config, bind_ip));
        return Ok(());
    }

    let _logging_guard = logging::init(&config.logging).context("не удалось настроить logging")?;

    screen_bridge_host::run(config)
}
