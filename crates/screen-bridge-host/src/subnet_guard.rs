//! Проверка `allow_subnet` для входящих RTSP clients.

use anyhow::{Context, Result};
use screen_bridge_core::net::{self, Subnet};

use crate::peer_ip::PeerAddress;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SubnetGuard {
    config_value: String,
    subnet: Subnet,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum SubnetDecision {
    Allow,
    Reject { reason: String },
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

    pub(crate) fn is_any(&self) -> bool {
        matches!(self.subnet, Subnet::Any)
    }

    pub(crate) fn check_peer(&self, peer_address: &PeerAddress) -> SubnetDecision {
        match (&self.subnet, peer_address) {
            (Subnet::Any, _) => SubnetDecision::Allow,
            (Subnet::V4(_), PeerAddress::Unavailable) => SubnetDecision::Reject {
                reason: format!(
                    "peer IP is unavailable, cannot enforce security.allow_subnet = \"{}\"",
                    self.config_value
                ),
            },
            (Subnet::V4(_), PeerAddress::Unsupported(value)) => SubnetDecision::Reject {
                reason: format!(
                    "peer address \"{value}\" is not supported by IPv4 security.allow_subnet = \"{}\"",
                    self.config_value
                ),
            },
            (Subnet::V4(_), PeerAddress::V4(ip)) if net::matches_subnet(*ip, &self.subnet) => {
                SubnetDecision::Allow
            }
            (Subnet::V4(_), PeerAddress::V4(ip)) => SubnetDecision::Reject {
                reason: format!(
                    "peer IP {ip} is outside security.allow_subnet = \"{}\"",
                    self.config_value
                ),
            },
        }
    }

    pub(crate) fn log_startup_warning_if_any(&self) {
        if self.is_any() {
            tracing::warn!(
                "security.allow_subnet = \"any\"; subnet filtering is disabled by explicit config"
            );
        }
    }

    pub(crate) fn log_client_warning_if_any(&self, peer_address: &PeerAddress) {
        if self.is_any() {
            tracing::warn!(
                peer = %peer_address,
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
    fn guard_should_accept_any_for_unavailable_peer() {
        // Given
        let guard = SubnetGuard::new("any").unwrap();

        // When
        let result = guard.check_peer(&PeerAddress::Unavailable);

        // Then
        assert_eq!(result, SubnetDecision::Allow);
    }

    #[test]
    fn guard_should_allow_peer_inside_cidr() {
        // Given
        let guard = SubnetGuard::new("192.168.1.0/24").unwrap();

        // When
        let result = guard.check_peer(&PeerAddress::V4(Ipv4Addr::new(192, 168, 1, 42)));

        // Then
        assert_eq!(result, SubnetDecision::Allow);
    }

    #[test]
    fn guard_should_reject_peer_outside_cidr() {
        // Given
        let guard = SubnetGuard::new("192.168.1.0/24").unwrap();

        // When
        let result = guard.check_peer(&PeerAddress::V4(Ipv4Addr::new(192, 168, 2, 42)));

        // Then
        assert!(matches!(result, SubnetDecision::Reject { .. }));
    }

    #[test]
    fn guard_should_reject_unavailable_peer_for_cidr() {
        // Given
        let guard = SubnetGuard::new("192.168.1.0/24").unwrap();

        // When
        let result = guard.check_peer(&PeerAddress::Unavailable);

        // Then
        assert!(matches!(result, SubnetDecision::Reject { .. }));
    }
}
