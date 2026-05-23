//! Проверка `allow_subnet` для входящих RTSP clients.

use anyhow::{bail, Context, Result};
use screen_bridge_core::net::{self, Subnet};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SubnetGuard {
    config_value: String,
    subnet: Subnet,
}

impl SubnetGuard {
    pub(crate) fn new(config_value: &str) -> Result<Self> {
        let subnet = net::parse_subnet(config_value).with_context(|| {
            format!("не удалось разобрать security.allow_subnet `{config_value}`")
        })?;

        Ok(Self {
            config_value: config_value.to_owned(),
            subnet,
        })
    }

    pub(crate) fn ensure_supported_by_safe_api(&self) -> Result<()> {
        if self.is_any() {
            return Ok(());
        }

        bail!(
            "security.allow_subnet = \"{}\" требует peer IP, но safe Rust bindings \
             gstreamer-rtsp-server 0.25.2 не раскрывают RTSPClient connection; \
             временно задайте allow_subnet = \"any\" для явного opt-out или примите \
             отдельное решение по минимальному FFI",
            self.config_value
        )
    }

    pub(crate) fn is_any(&self) -> bool {
        matches!(self.subnet, Subnet::Any)
    }

    #[cfg(test)]
    fn check_peer(&self, peer_ip: std::net::Ipv4Addr) -> bool {
        net::matches_subnet(peer_ip, &self.subnet)
    }

    pub(crate) fn log_startup_warning_if_any(&self) {
        if self.is_any() {
            tracing::warn!(
                "security.allow_subnet = \"any\"; subnet filtering is disabled by explicit config"
            );
        }
    }

    pub(crate) fn log_client_warning_if_any(&self) {
        if self.is_any() {
            tracing::warn!(
                "RTSP client connection observed; security.allow_subnet = \"any\" accepts clients without subnet filtering"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn guard_should_accept_any_without_startup_blocker() {
        // Given
        let guard = SubnetGuard::new("any").unwrap();

        // When
        let result = guard.ensure_supported_by_safe_api();

        // Then
        assert!(result.is_ok());
        assert!(guard.check_peer(Ipv4Addr::new(203, 0, 113, 10)));
    }

    #[test]
    fn guard_should_block_cidr_until_peer_ip_safe_api_exists() {
        // Given
        let guard = SubnetGuard::new("192.168.1.0/24").unwrap();

        // When
        let result = guard.ensure_supported_by_safe_api();

        // Then
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("RTSPClient connection"));
    }

    #[test]
    fn guard_should_match_peer_against_cidr() {
        // Given
        let guard = SubnetGuard::new("192.168.1.0/24").unwrap();

        // When / Then
        assert!(guard.check_peer(Ipv4Addr::new(192, 168, 1, 42)));
        assert!(!guard.check_peer(Ipv4Addr::new(192, 168, 2, 42)));
    }
}
