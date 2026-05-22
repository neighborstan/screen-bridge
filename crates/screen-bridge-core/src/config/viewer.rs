//! Viewer config schema и validation.

use serde::Deserialize;

use crate::config::validate::{
    invalid, validate_access_token, validate_logging, validate_non_empty, validate_one_of,
    validate_port, validate_positive_u32, validate_stream_path,
};
use crate::config::{ConfigError, LoggingConfig};
use crate::secret::Secret;

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
/// Полный config для `screen-bridge-viewer`.
pub struct ViewerConfig {
    /// Настройки подключения к host.
    pub connection: ConnectionConfig,
    /// Настройки RTSP playback.
    pub playback: PlaybackConfig,
    /// Настройки логирования.
    pub logging: LoggingConfig,
}

impl Default for ViewerConfig {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig::default(),
            playback: PlaybackConfig::default(),
            logging: LoggingConfig {
                file_prefix: "viewer".to_owned(),
                ..LoggingConfig::default()
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
/// Настройки подключения viewer к host.
pub struct ConnectionConfig {
    /// IPv4 или hostname host machine.
    pub host: String,
    /// RTSP port host machine.
    pub port: u16,
    /// RTSP path, например `/screen`.
    pub stream_path: String,
    /// Имя пользователя для RTSP Basic auth.
    pub auth_user: String,
    /// Token для RTSP Basic auth.
    pub access_token: Secret,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            host: "192.168.1.25".to_owned(),
            port: 8554,
            stream_path: "/screen".to_owned(),
            auth_user: "viewer".to_owned(),
            access_token: Secret::new("change-me-please-16"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
/// Настройки воспроизведения RTSP stream.
pub struct PlaybackConfig {
    /// Transport для RTSP. В MVP разрешен только "tcp".
    pub rtsp_transport: String,
    /// GStreamer latency buffer в миллисекундах.
    pub latency_ms: u32,
    /// Автопереподключение. В MVP должно быть `false`.
    pub reconnect: bool,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            rtsp_transport: "tcp".to_owned(),
            latency_ms: 100,
            reconnect: false,
        }
    }
}

/// Проверяет viewer config по правилам MVP.
pub fn validate_viewer(config: ViewerConfig) -> Result<ViewerConfig, ConfigError> {
    validate_non_empty("connection.host", &config.connection.host)?;
    validate_port("connection.port", config.connection.port)?;
    validate_stream_path("connection.stream_path", &config.connection.stream_path)?;
    validate_non_empty("connection.auth_user", &config.connection.auth_user)?;
    validate_access_token(&config.connection.access_token)?;

    validate_one_of(
        "playback.rtsp_transport",
        &config.playback.rtsp_transport,
        &["tcp"],
    )?;
    validate_positive_u32("playback.latency_ms", config.playback.latency_ms)?;

    if config.playback.reconnect {
        return invalid(
            "playback.reconnect",
            "MVP поддерживает только reconnect = false",
        );
    }

    validate_logging(&config.logging)?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{load_viewer, parse_viewer_toml};

    fn valid_viewer_config() -> ViewerConfig {
        let mut config = ViewerConfig::default();
        config.connection.access_token = Secret::new("valid-token-1234");
        config
    }

    #[test]
    fn parse_viewer_toml_should_accept_example_config() {
        // Given
        let toml = include_str!("../../../../config/viewer.example.toml");

        // When
        let result = parse_viewer_toml(toml);

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn parse_viewer_toml_should_apply_defaults() {
        // Given
        let toml = "";

        // When
        let result = parse_viewer_toml(toml).unwrap();

        // Then
        assert_eq!(result.connection.host, "192.168.1.25");
        assert_eq!(result.connection.port, 8554);
        assert_eq!(result.connection.stream_path, "/screen");
        assert_eq!(result.connection.auth_user, "viewer");
        assert_eq!(result.playback.rtsp_transport, "tcp");
        assert_eq!(result.playback.latency_ms, 100);
        assert!(!result.playback.reconnect);
        assert_eq!(result.logging.level, "info");
        assert_eq!(result.logging.file_prefix, "viewer");
    }

    #[test]
    fn load_viewer_should_reject_example_placeholder_token() {
        // Given
        let path = example_config_path("viewer.example.toml");

        // When
        let result = load_viewer(path);

        // Then
        let error = result.unwrap_err().to_string();
        assert!(error.contains("измените `access_token`"));
    }

    #[test]
    fn validate_viewer_should_accept_valid_config() {
        // Given
        let config = valid_viewer_config();

        // When
        let result = validate_viewer(config);

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn validate_viewer_should_reject_invalid_connection_values() {
        assert_invalid_field(
            with_viewer_config(|config| config.connection.host.clear()),
            "connection.host",
        );
        assert_invalid_field(
            with_viewer_config(|config| config.connection.port = 0),
            "connection.port",
        );
        assert_invalid_field(
            with_viewer_config(|config| config.connection.stream_path = "screen".to_owned()),
            "connection.stream_path",
        );
        assert_invalid_field(
            with_viewer_config(|config| config.connection.auth_user.clear()),
            "connection.auth_user",
        );
    }

    #[test]
    fn validate_viewer_should_reject_invalid_token() {
        // Given
        let mut config = valid_viewer_config();
        config.connection.access_token = Secret::new("change-me-please-16");

        // When
        let result = validate_viewer(config);

        // Then
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("измените `access_token`"));
    }

    #[test]
    fn validate_viewer_should_reject_invalid_playback_values() {
        assert_invalid_field(
            with_viewer_config(|config| config.playback.rtsp_transport = "udp".to_owned()),
            "playback.rtsp_transport",
        );
        assert_invalid_field(
            with_viewer_config(|config| config.playback.latency_ms = 0),
            "playback.latency_ms",
        );
        assert_invalid_field(
            with_viewer_config(|config| config.playback.reconnect = true),
            "playback.reconnect",
        );
    }

    fn with_viewer_config(
        mutate: impl FnOnce(&mut ViewerConfig),
    ) -> Result<ViewerConfig, ConfigError> {
        let mut config = valid_viewer_config();
        mutate(&mut config);
        validate_viewer(config)
    }

    fn assert_invalid_field(result: Result<ViewerConfig, ConfigError>, field: &str) {
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains(field),
            "error `{error}` should contain `{field}`"
        );
    }

    fn example_config_path(file_name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("config")
            .join(file_name)
    }
}
