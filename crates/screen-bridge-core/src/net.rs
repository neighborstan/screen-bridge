use std::net::Ipv4Addr;
use std::str::FromStr;

use if_addrs::{get_if_addrs, IfAddr};
use ipnet::Ipv4Net;
use thiserror::Error;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Subnet {
    Any,
    V4(Ipv4Net),
}

impl Subnet {
    pub fn matches(&self, ip: Ipv4Addr) -> bool {
        match self {
            Self::Any => true,
            Self::V4(net) => net.contains(&ip),
        }
    }
}

#[derive(Debug, Error)]
pub enum SubnetParseError {
    #[error("значение обязательно: используйте IPv4 CIDR или \"any\"")]
    Empty,
    #[error("поддерживается только IPv4 CIDR или \"any\"")]
    Invalid { reason: String },
}

#[derive(Debug, Error)]
pub enum LocalIpError {
    #[error("не удалось получить локальные IPv4 interfaces: {0}")]
    Query(#[from] std::io::Error),
}

pub fn parse_subnet(value: &str) -> Result<Subnet, SubnetParseError> {
    let value = value.trim();

    if value.is_empty() {
        return Err(SubnetParseError::Empty);
    }

    if value.eq_ignore_ascii_case("any") {
        return Ok(Subnet::Any);
    }

    Ipv4Net::from_str(value)
        .map(Subnet::V4)
        .map_err(|error| SubnetParseError::Invalid {
            reason: error.to_string(),
        })
}

pub fn matches_subnet(ip: Ipv4Addr, subnet: &Subnet) -> bool {
    subnet.matches(ip)
}

pub fn local_ipv4() -> Result<Vec<Ipv4Addr>, LocalIpError> {
    let mut addresses = Vec::new();

    for interface in get_if_addrs()? {
        if interface.is_loopback() {
            continue;
        }

        let IfAddr::V4(address) = interface.addr else {
            continue;
        };

        if is_usable_lan_ipv4(address.ip) {
            addresses.push(address.ip);
        }
    }

    addresses.sort_unstable();
    addresses.dedup();

    Ok(addresses)
}

pub fn is_usable_lan_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_private()
        && !ip.is_loopback()
        && !ip.is_unspecified()
        && !ip.is_multicast()
        && !ip.is_broadcast()
        && !ip.is_link_local()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_subnet_should_accept_any() {
        // Given
        let value = "any";

        // When
        let result = parse_subnet(value).unwrap();

        // Then
        assert_eq!(result, Subnet::Any);
    }

    #[test]
    fn parse_subnet_should_accept_ipv4_cidr() {
        // Given
        let value = "192.168.1.0/24";

        // When
        let result = parse_subnet(value).unwrap();

        // Then
        assert!(matches!(result, Subnet::V4(_)));
    }

    #[test]
    fn parse_subnet_should_reject_invalid_cidr() {
        // Given
        let value = "not-cidr";

        // When
        let result = parse_subnet(value);

        // Then
        assert!(result.is_err());
    }

    #[test]
    fn matches_subnet_should_match_ip_inside_cidr() {
        // Given
        let subnet = parse_subnet("192.168.1.0/24").unwrap();

        // When
        let result = matches_subnet(Ipv4Addr::new(192, 168, 1, 42), &subnet);

        // Then
        assert!(result);
    }

    #[test]
    fn matches_subnet_should_reject_ip_outside_cidr() {
        // Given
        let subnet = parse_subnet("192.168.1.0/24").unwrap();

        // When
        let result = matches_subnet(Ipv4Addr::new(192, 168, 2, 42), &subnet);

        // Then
        assert!(!result);
    }

    #[test]
    fn matches_subnet_should_accept_everything_for_any() {
        // Given
        let subnet = Subnet::Any;

        // When
        let result = matches_subnet(Ipv4Addr::new(203, 0, 113, 10), &subnet);

        // Then
        assert!(result);
    }

    #[test]
    fn is_usable_lan_ipv4_should_filter_unsuitable_addresses() {
        assert!(!is_usable_lan_ipv4(Ipv4Addr::new(127, 0, 0, 1)));
        assert!(!is_usable_lan_ipv4(Ipv4Addr::new(0, 0, 0, 0)));
        assert!(!is_usable_lan_ipv4(Ipv4Addr::new(224, 0, 0, 1)));
        assert!(!is_usable_lan_ipv4(Ipv4Addr::new(255, 255, 255, 255)));
        assert!(!is_usable_lan_ipv4(Ipv4Addr::new(169, 254, 1, 10)));
        assert!(!is_usable_lan_ipv4(Ipv4Addr::new(8, 8, 8, 8)));
        assert!(is_usable_lan_ipv4(Ipv4Addr::new(192, 168, 1, 25)));
    }

    #[test]
    #[ignore = "manual check depends on current machine network interfaces"]
    fn local_ipv4_manual_should_return_non_empty_on_dev_machine() {
        // Given
        let addresses = local_ipv4().unwrap();

        // When
        let has_private_lan_ipv4 = !addresses.is_empty();

        // Then
        assert!(
            has_private_lan_ipv4,
            "expected at least one private LAN IPv4, found none"
        );
    }
}
