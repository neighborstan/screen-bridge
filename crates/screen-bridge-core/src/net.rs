//! Помощники для LAN IPv4 и subnet allowlist.

use std::io::ErrorKind;
use std::net::{Ipv4Addr, TcpStream, ToSocketAddrs};
use std::str::FromStr;
use std::time::Duration;

use if_addrs::{get_if_addrs, IfAddr};
use ipnet::Ipv4Net;
use thiserror::Error;

#[derive(Debug, Clone, Eq, PartialEq)]
/// Разрешенная subnet для входящих подключений.
pub enum Subnet {
    /// Явный opt-out: разрешить любой peer IP.
    Any,
    /// IPv4 CIDR, например `192.168.1.0/24`.
    V4(Ipv4Net),
}

impl Subnet {
    /// Проверяет, входит ли IPv4 address в эту subnet.
    pub fn matches(&self, ip: Ipv4Addr) -> bool {
        match self {
            Self::Any => true,
            Self::V4(net) => net.contains(&ip),
        }
    }
}

#[derive(Debug, Error)]
/// Ошибка разбора subnet из config.
pub enum SubnetParseError {
    /// Значение пустое.
    #[error("значение обязательно: используйте IPv4 CIDR или \"any\"")]
    Empty,
    /// Значение не похоже на IPv4 CIDR или "any".
    #[error("поддерживается только IPv4 CIDR или \"any\"")]
    Invalid {
        /// Причина от parser.
        reason: String,
    },
}

#[derive(Debug, Error)]
/// Ошибка получения локальных IPv4 addresses.
pub enum LocalIpError {
    /// OS не вернула список network interfaces.
    #[error("не удалось получить локальные IPv4 interfaces: {0}")]
    Query(#[from] std::io::Error),
}

#[derive(Debug, Error)]
/// Ошибка TCP preflight подключения к host.
pub enum TcpPreflightError {
    /// Hostname или address не удалось resolved в socket addresses.
    #[error("не удалось resolved TCP target {host}:{port}: {source}")]
    Resolve {
        /// Hostname или IP из config.
        host: String,
        /// TCP port из config.
        port: u16,
        /// Ошибка OS resolver.
        source: std::io::Error,
    },
    /// Resolver не вернул ни одного socket address.
    #[error("TCP target {host}:{port} не дал socket addresses")]
    NoAddresses {
        /// Hostname или IP из config.
        host: String,
        /// TCP port из config.
        port: u16,
    },
    /// TCP connect не прошел ни к одному resolved address.
    #[error("TCP connect to {host}:{port} failed within {timeout_ms} ms: {attempts}")]
    Connect {
        /// Hostname или IP из config.
        host: String,
        /// TCP port из config.
        port: u16,
        /// Timeout одной TCP connect попытки.
        timeout_ms: u128,
        /// Категория TCP connect failure для user-facing диагностики.
        kind: TcpPreflightConnectKind,
        /// Ошибки по resolved socket addresses.
        attempts: String,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// Категория сбоя TCP preflight подключения.
pub enum TcpPreflightConnectKind {
    /// Endpoint явно отказал в соединении.
    Refused,
    /// Connect не получил ответа до timeout.
    Timeout,
    /// Сеть или host недоступны по маршрутизации.
    Unreachable,
    /// OS вернула другую ошибку.
    Other,
}

/// Разбирает `allow_subnet` из config.
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

/// Проверяет IPv4 address по subnet allowlist.
pub fn matches_subnet(ip: Ipv4Addr, subnet: &Subnet) -> bool {
    subnet.matches(ip)
}

/// Возвращает локальные private LAN IPv4 addresses.
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

/// Проверяет, что TCP endpoint отвечает до запуска более дорогого playback.
pub fn check_tcp_connect(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<(), TcpPreflightError> {
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|source| TcpPreflightError::Resolve {
            host: host.to_owned(),
            port,
            source,
        })?
        .collect::<Vec<_>>();

    if addresses.is_empty() {
        return Err(TcpPreflightError::NoAddresses {
            host: host.to_owned(),
            port,
        });
    }

    let mut attempts = Vec::new();
    let mut error_kinds = Vec::new();
    for address in addresses {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(_) => return Ok(()),
            Err(error) => {
                error_kinds.push(error.kind());
                attempts.push(format!("{address}: {error}"));
            }
        }
    }

    Err(TcpPreflightError::Connect {
        host: host.to_owned(),
        port,
        timeout_ms: timeout.as_millis(),
        kind: classify_tcp_connect_failure(&error_kinds),
        attempts: attempts.join("; "),
    })
}

fn classify_tcp_connect_failure(kinds: &[ErrorKind]) -> TcpPreflightConnectKind {
    if kinds.iter().any(|kind| matches!(kind, ErrorKind::TimedOut)) {
        return TcpPreflightConnectKind::Timeout;
    }

    if kinds
        .iter()
        .any(|kind| matches!(kind, ErrorKind::ConnectionRefused))
    {
        return TcpPreflightConnectKind::Refused;
    }

    if kinds.iter().all(|kind| {
        matches!(
            kind,
            ErrorKind::AddrNotAvailable
                | ErrorKind::HostUnreachable
                | ErrorKind::NetworkDown
                | ErrorKind::NetworkUnreachable
        )
    }) {
        return TcpPreflightConnectKind::Unreachable;
    }

    TcpPreflightConnectKind::Other
}

/// Проверяет, подходит ли IPv4 address для автоматического bind в LAN.
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
    use std::net::TcpListener;

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
    fn check_tcp_connect_should_accept_local_listener() {
        // Given
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();

        // When
        let result = check_tcp_connect("127.0.0.1", port, Duration::from_millis(500));

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn classify_tcp_connect_failure_should_prefer_timeout() {
        // Given
        let kinds = [ErrorKind::NetworkUnreachable, ErrorKind::TimedOut];

        // When
        let result = classify_tcp_connect_failure(&kinds);

        // Then
        assert_eq!(result, TcpPreflightConnectKind::Timeout);
    }

    #[test]
    fn classify_tcp_connect_failure_should_detect_refused_port() {
        // Given
        let kinds = [ErrorKind::ConnectionRefused];

        // When
        let result = classify_tcp_connect_failure(&kinds);

        // Then
        assert_eq!(result, TcpPreflightConnectKind::Refused);
    }

    #[test]
    fn classify_tcp_connect_failure_should_detect_unreachable_host() {
        // Given
        let kinds = [ErrorKind::HostUnreachable, ErrorKind::NetworkUnreachable];

        // When
        let result = classify_tcp_connect_failure(&kinds);

        // Then
        assert_eq!(result, TcpPreflightConnectKind::Unreachable);
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
