//! Формирование RTSP URL для host startup, diagnostics и VLC.

use std::net::Ipv4Addr;

use screen_bridge_core::config::HostConfig;

/// Строит полный RTSP URL с настоящим token для явного режима `--print-vlc-url`.
pub fn build_vlc_url(config: &HostConfig, bind_ip: Ipv4Addr) -> String {
    build_rtsp_url(
        &config.security.auth_user,
        config.security.access_token.as_str(),
        bind_ip,
        config.server.port,
        &config.server.stream_path,
    )
}

/// Строит RTSP URL с замаскированным token для stdout, логов и diagnostics.
pub fn build_masked_rtsp_url(config: &HostConfig, bind_ip: Ipv4Addr) -> String {
    build_rtsp_url(
        &config.security.auth_user,
        &config.security.access_token.masked(),
        bind_ip,
        config.server.port,
        &config.server.stream_path,
    )
}

fn build_rtsp_url(
    auth_user: &str,
    access_token: &str,
    bind_ip: Ipv4Addr,
    port: u16,
    stream_path: &str,
) -> String {
    format!(
        "rtsp://{}:{}@{}:{}{}",
        percent_encode(auth_user, false, false),
        percent_encode(access_token, false, true),
        bind_ip,
        port,
        percent_encode(stream_path, true, false)
    )
}

fn percent_encode(value: &str, preserve_slash: bool, preserve_asterisk: bool) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        if is_unreserved(byte)
            || (preserve_slash && byte == b'/')
            || (preserve_asterisk && byte == b'*')
        {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use screen_bridge_core::config::HostConfig;
    use screen_bridge_core::secret::Secret;

    use super::*;

    fn valid_config() -> HostConfig {
        let mut config = HostConfig::default();
        config.security.access_token = Secret::new("valid-token-1234");
        config
    }

    #[test]
    fn build_vlc_url_should_include_credentials_and_stream_path() {
        // Given
        let config = valid_config();

        // When
        let url = build_vlc_url(&config, Ipv4Addr::new(192, 168, 1, 25));

        // Then
        assert_eq!(
            url,
            "rtsp://viewer:valid-token-1234@192.168.1.25:8554/screen"
        );
    }

    #[test]
    fn build_vlc_url_should_percent_encode_userinfo_and_path() {
        // Given
        let mut config = valid_config();
        config.security.auth_user = "view@er".to_owned();
        config.security.access_token = Secret::new("token:with/slash");
        config.server.stream_path = "/screen one".to_owned();

        // When
        let url = build_vlc_url(&config, Ipv4Addr::new(192, 168, 1, 25));

        // Then
        assert_eq!(
            url,
            "rtsp://view%40er:token%3Awith%2Fslash@192.168.1.25:8554/screen%20one"
        );
    }

    #[test]
    fn build_masked_rtsp_url_should_not_expose_full_token() {
        // Given
        let config = valid_config();

        // When
        let url = build_masked_rtsp_url(&config, Ipv4Addr::new(192, 168, 1, 25));

        // Then
        assert!(!url.contains(config.security.access_token.as_str()));
        assert!(url.contains("val**********234"));
    }
}
