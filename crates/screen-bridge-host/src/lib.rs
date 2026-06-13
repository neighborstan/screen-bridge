//! Host runtime для публикации screen stream через RTSP.

#![warn(missing_docs)]

mod auth;
mod diagnose;
mod peer_ip;
mod pipeline;
mod server;
mod session_limit;
mod subnet_guard;
mod url;

use anyhow::{Context, Result};
use screen_bridge_core::config::HostConfig;
use screen_bridge_core::net;

pub use diagnose::{diagnose, DiagnosticReport};
pub use url::{build_masked_rtsp_url, build_vlc_url};

use crate::pipeline::GstElementAvailability;
use crate::server::HostServer;
use crate::subnet_guard::SubnetGuard;

/// Запускает host RTSP server по уже загруженному и проверенному config.
pub fn run(config: HostConfig) -> Result<()> {
    let subnet_guard = SubnetGuard::new(&config.security.allow_subnet)?;
    gstreamer::init().context("не удалось инициализировать GStreamer")?;

    let bind_ip = resolve_bind_ip(&config)?;
    let availability = GstElementAvailability;
    let launch = pipeline::build_launch_string(&config.video, &config.capture, &availability)?;
    let server = HostServer::start(
        bind_ip,
        &config.server,
        &config.security,
        &subnet_guard,
        &launch,
    )?;
    subnet_guard.log_startup_warning_if_any();

    println!("ScreenBridge host is ready.");
    println!("Bind: {}:{}", server.bind_ip(), server.port());
    println!("Path: {}", server.stream_path());
    println!(
        "RTSP URL: {}",
        build_masked_rtsp_url(&config, server.bind_ip())
    );
    println!(
        "Auth: Basic user={}, token={}",
        config.security.auth_user, config.security.access_token
    );
    println!(
        "Firewall: if remote viewers cannot connect, use Start Menu shortcut \"ScreenBridge Allow Host Firewall\" or allow inbound TCP port {}.",
        server.port()
    );
    println!("Press Ctrl+C to stop.");

    server.run_until_ctrl_c()?;

    Ok(())
}

/// Возвращает IPv4, на котором host должен слушать RTSP.
pub fn resolve_bind_ip(config: &HostConfig) -> Result<std::net::Ipv4Addr> {
    if let Some(bind_ip) = config.server.bind_ip {
        return Ok(bind_ip);
    }

    let addresses = net::local_ipv4().context("не удалось получить локальные LAN IPv4")?;
    choose_bind_ip(&addresses)
        .context("не найден LAN IPv4 для bind; задайте server.bind_ip в host config явно")
}

fn choose_bind_ip(addresses: &[std::net::Ipv4Addr]) -> Option<std::net::Ipv4Addr> {
    addresses
        .iter()
        .copied()
        .min_by_key(|ip| bind_ip_priority(*ip))
}

fn bind_ip_priority(ip: std::net::Ipv4Addr) -> u8 {
    let octets = ip.octets();
    match octets {
        [192, 168, _, _] => 0,
        [172, second, _, _] if (16..=31).contains(&second) => 1,
        [10, _, _, _] => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn choose_bind_ip_should_prefer_home_lan_over_vpn_like_addresses() {
        // Given
        let addresses = [
            Ipv4Addr::new(10, 34, 242, 190),
            Ipv4Addr::new(172, 20, 176, 1),
            Ipv4Addr::new(192, 168, 1, 151),
        ];

        // When
        let result = choose_bind_ip(&addresses).unwrap();

        // Then
        assert_eq!(result, Ipv4Addr::new(192, 168, 1, 151));
    }

    #[test]
    fn choose_bind_ip_should_return_none_for_empty_list() {
        // Given
        let addresses = [];

        // When
        let result = choose_bind_ip(&addresses);

        // Then
        assert_eq!(result, None);
    }
}
