//! Получение peer address из `gst-rtsp-server`.

use std::ffi::CStr;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr};

use glib::translate::ToGlibPtr;
use gstreamer_rtsp_server::{ffi, gst_rtsp, RTSPClient};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum PeerAddress {
    V4(Ipv4Addr),
    Unsupported(String),
    Unavailable,
}

impl fmt::Display for PeerAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V4(ip) => write!(formatter, "{ip}"),
            Self::Unsupported(value) => write!(formatter, "{value}"),
            Self::Unavailable => formatter.write_str("<unavailable>"),
        }
    }
}

pub(crate) fn client_peer_address(client: &RTSPClient) -> PeerAddress {
    // SAFETY: `client` - живой `RTSPClient`, взятый из GStreamer signal.
    // `gst_rtsp_client_get_connection` возвращает borrowed connection pointer,
    // а `gst_rtsp_connection_get_ip` - NUL-terminated строку, которой владеет
    // connection. Мы копируем строку до выхода из функции.
    unsafe {
        let connection = ffi::gst_rtsp_client_get_connection(client.to_glib_none().0);
        if connection.is_null() {
            return PeerAddress::Unavailable;
        }

        let raw_ip = gst_rtsp::ffi::gst_rtsp_connection_get_ip(connection);
        if raw_ip.is_null() {
            return PeerAddress::Unavailable;
        }

        match CStr::from_ptr(raw_ip).to_str() {
            Ok(value) => parse_peer_address(value),
            Err(_) => PeerAddress::Unsupported("<non-utf8>".to_owned()),
        }
    }
}

fn parse_peer_address(value: &str) -> PeerAddress {
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => PeerAddress::V4(ip),
        Ok(IpAddr::V6(_)) | Err(_) => PeerAddress::Unsupported(value.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_peer_address_should_accept_ipv4() {
        // Given / When
        let result = parse_peer_address("192.168.1.42");

        // Then
        assert_eq!(result, PeerAddress::V4(Ipv4Addr::new(192, 168, 1, 42)));
    }

    #[test]
    fn parse_peer_address_should_mark_ipv6_as_unsupported() {
        // Given / When
        let result = parse_peer_address("::1");

        // Then
        assert_eq!(result, PeerAddress::Unsupported("::1".to_owned()));
    }

    #[test]
    fn parse_peer_address_should_mark_invalid_value_as_unsupported() {
        // Given / When
        let result = parse_peer_address("not-an-ip");

        // Then
        assert_eq!(result, PeerAddress::Unsupported("not-an-ip".to_owned()));
    }
}
