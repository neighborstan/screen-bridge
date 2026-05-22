//! Host config schema и validation.

use std::net::Ipv4Addr;

use serde::Deserialize;

use crate::config::validate::{
    invalid, validate_access_token, validate_logging, validate_non_empty, validate_one_of,
    validate_port, validate_positive_u32, validate_stream_path, ConfigWarning,
};
use crate::config::{ConfigError, LoggingConfig};
use crate::net::{self, Subnet};
use crate::secret::Secret;

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
/// Полный config для `screen-bridge-host`.
pub struct HostConfig {
    /// Настройки RTSP server.
    pub server: ServerConfig,
    /// Настройки доступа к stream.
    pub security: SecurityConfig,
    /// Настройки размера, fps и encoder mode.
    pub video: VideoConfig,
    /// Настройки захвата экрана.
    pub capture: CaptureConfig,
    /// Настройки логирования.
    pub logging: LoggingConfig,
    /// Предупреждения после validation.
    #[serde(skip)]
    pub warnings: Vec<ConfigWarning>,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            security: SecurityConfig::default(),
            video: VideoConfig::default(),
            capture: CaptureConfig::default(),
            logging: LoggingConfig {
                file_prefix: "host".to_owned(),
                ..LoggingConfig::default()
            },
            warnings: Vec::new(),
        }
    }
}

impl HostConfig {
    /// Проверяет, есть ли warning после validation.
    pub fn has_warning(&self, warning: ConfigWarning) -> bool {
        self.warnings.contains(&warning)
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
/// Настройки RTSP server на host side.
pub struct ServerConfig {
    /// IPv4, на котором слушает host. Если не задан, host выберет LAN IPv4 сам.
    pub bind_ip: Option<Ipv4Addr>,
    /// TCP port RTSP server.
    pub port: u16,
    /// RTSP path, например `/screen`.
    pub stream_path: String,
    /// Максимум одновременных viewer для MVP.
    pub max_clients: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_ip: None,
            port: 8554,
            stream_path: "/screen".to_owned(),
            max_clients: 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
/// Настройки Basic auth и subnet allowlist.
pub struct SecurityConfig {
    /// Имя пользователя для RTSP Basic auth.
    pub auth_user: String,
    /// Token для RTSP Basic auth.
    pub access_token: Secret,
    /// Разрешенная IPv4 subnet или явное значение "any".
    pub allow_subnet: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            auth_user: "viewer".to_owned(),
            access_token: Secret::new("change-me-please-16"),
            allow_subnet: "192.168.1.0/24".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
/// Настройки видеопотока host.
pub struct VideoConfig {
    /// Ширина кадра.
    pub width: u32,
    /// Высота кадра.
    pub height: u32,
    /// Частота кадров.
    pub fps: u32,
    /// Целевой bitrate encoder в kbps.
    pub bitrate_kbps: u32,
    /// Режим выбора encoder: "auto" или "software_only".
    pub encoder: String,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 15,
            bitrate_kbps: 2500,
            encoder: "auto".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
/// Настройки захвата экрана.
pub struct CaptureConfig {
    /// Индекс монитора. Значение -1 означает primary monitor.
    pub monitor_index: i32,
    /// Нужно ли захватывать курсор.
    pub capture_cursor: bool,
    /// Capture backend для GStreamer source: "dxgi" или "wgc".
    pub capture_api: String,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            monitor_index: -1,
            capture_cursor: true,
            capture_api: "dxgi".to_owned(),
        }
    }
}

/// Проверяет host config и возвращает его вместе с warning flags.
pub fn validate_host(mut config: HostConfig) -> Result<HostConfig, ConfigError> {
    config.warnings.clear();

    validate_stream_path("server.stream_path", &config.server.stream_path)?;
    validate_port("server.port", config.server.port)?;

    if config.server.max_clients != 1 {
        return invalid(
            "server.max_clients",
            "MVP поддерживает только max_clients = 1",
        );
    }

    validate_non_empty("security.auth_user", &config.security.auth_user)?;
    validate_access_token(&config.security.access_token)?;
    let allow_subnet = validate_allow_subnet(&config.security.allow_subnet)?;
    if matches!(allow_subnet, Subnet::Any) {
        config.warnings.push(ConfigWarning::AllowSubnetAny);
    }

    validate_positive_u32("video.width", config.video.width)?;
    validate_positive_u32("video.height", config.video.height)?;
    validate_positive_u32("video.fps", config.video.fps)?;
    validate_positive_u32("video.bitrate_kbps", config.video.bitrate_kbps)?;
    validate_one_of(
        "video.encoder",
        &config.video.encoder,
        &["auto", "software_only"],
    )?;

    if config.capture.monitor_index < -1 {
        return invalid(
            "capture.monitor_index",
            "значение -1 означает primary monitor, допустимые явные индексы начинаются с 0",
        );
    }
    validate_one_of(
        "capture.capture_api",
        &config.capture.capture_api,
        &["dxgi", "wgc"],
    )?;
    validate_logging(&config.logging)?;

    Ok(config)
}

fn validate_allow_subnet(value: &str) -> Result<Subnet, ConfigError> {
    net::parse_subnet(value).map_err(|source| ConfigError::InvalidSubnet {
        value: value.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{load_host, parse_host_toml};

    fn valid_host_config() -> HostConfig {
        let mut config = HostConfig::default();
        config.security.access_token = Secret::new("valid-token-1234");
        config
    }

    #[test]
    fn parse_host_toml_should_accept_example_config() {
        // Given
        let toml = include_str!("../../../../config/host.example.toml");

        // When
        let result = parse_host_toml(toml);

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn parse_host_toml_should_apply_defaults() {
        // Given
        let toml = "";

        // When
        let result = parse_host_toml(toml).unwrap();

        // Then
        assert_eq!(result.server.port, 8554);
        assert_eq!(result.server.stream_path, "/screen");
        assert_eq!(result.server.max_clients, 1);
        assert_eq!(result.security.auth_user, "viewer");
        assert_eq!(result.security.allow_subnet, "192.168.1.0/24");
        assert_eq!(result.video.width, 1280);
        assert_eq!(result.video.height, 720);
        assert_eq!(result.video.fps, 15);
        assert_eq!(result.video.bitrate_kbps, 2500);
        assert_eq!(result.video.encoder, "auto");
        assert_eq!(result.capture.monitor_index, -1);
        assert!(result.capture.capture_cursor);
        assert_eq!(result.capture.capture_api, "dxgi");
        assert_eq!(result.logging.level, "info");
        assert_eq!(result.logging.file_prefix, "host");
    }

    #[test]
    fn load_host_should_reject_example_placeholder_token() {
        // Given
        let path = example_config_path("host.example.toml");

        // When
        let result = load_host(path);

        // Then
        let error = result.unwrap_err().to_string();
        assert!(error.contains("измените `access_token`"));
    }

    #[test]
    fn validate_host_should_accept_valid_config() {
        // Given
        let config = valid_host_config();

        // When
        let result = validate_host(config);

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn validate_host_should_reject_stream_path_without_slash() {
        // Given
        let mut config = valid_host_config();
        config.server.stream_path = "screen".to_owned();

        // When
        let result = validate_host(config);

        // Then
        assert_invalid_field(result, "server.stream_path");
    }

    #[test]
    fn validate_host_should_reject_zero_port() {
        // Given
        let mut config = valid_host_config();
        config.server.port = 0;

        // When
        let result = validate_host(config);

        // Then
        assert_invalid_field(result, "server.port");
    }

    #[test]
    fn validate_host_should_reject_max_clients_above_one() {
        // Given
        let mut config = valid_host_config();
        config.server.max_clients = 2;

        // When
        let result = validate_host(config);

        // Then
        assert_invalid_field(result, "server.max_clients");
    }

    #[test]
    fn validate_host_should_reject_empty_auth_user() {
        // Given
        let mut config = valid_host_config();
        config.security.auth_user.clear();

        // When
        let result = validate_host(config);

        // Then
        assert_invalid_field(result, "security.auth_user");
    }

    #[test]
    fn validate_host_should_reject_empty_token() {
        // Given
        let mut config = valid_host_config();
        config.security.access_token = Secret::new("");

        // When
        let result = validate_host(config);

        // Then
        assert_invalid_field(result, "access_token");
    }

    #[test]
    fn validate_host_should_reject_short_token() {
        // Given
        let mut config = valid_host_config();
        config.security.access_token = Secret::new("short-token");

        // When
        let result = validate_host(config);

        // Then
        assert_invalid_field(result, "access_token");
    }

    #[test]
    fn validate_host_should_reject_placeholder_token() {
        // Given
        let mut config = valid_host_config();
        config.security.access_token = Secret::new("change-me-please-16");

        // When
        let result = validate_host(config);

        // Then
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("измените `access_token`"));
    }

    #[test]
    fn validate_host_should_reject_invalid_allow_subnet() {
        // Given
        let mut config = valid_host_config();
        config.security.allow_subnet = "not-cidr".to_owned();

        // When
        let result = validate_host(config);

        // Then
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("security.allow_subnet"));
    }

    #[test]
    fn validate_host_should_accept_allow_subnet_any_with_warning() {
        // Given
        let mut config = valid_host_config();
        config.security.allow_subnet = "any".to_owned();

        // When
        let result = validate_host(config).unwrap();

        // Then
        assert!(result.has_warning(ConfigWarning::AllowSubnetAny));
    }

    #[test]
    fn validate_host_should_reject_invalid_video_values() {
        assert_invalid_field(
            with_host_config(|config| config.video.width = 0),
            "video.width",
        );
        assert_invalid_field(
            with_host_config(|config| config.video.height = 0),
            "video.height",
        );
        assert_invalid_field(with_host_config(|config| config.video.fps = 0), "video.fps");
        assert_invalid_field(
            with_host_config(|config| config.video.bitrate_kbps = 0),
            "video.bitrate_kbps",
        );
        assert_invalid_field(
            with_host_config(|config| config.video.encoder = "mfh264enc".to_owned()),
            "video.encoder",
        );
    }

    #[test]
    fn validate_host_should_reject_invalid_capture_values() {
        assert_invalid_field(
            with_host_config(|config| config.capture.monitor_index = -2),
            "capture.monitor_index",
        );
        assert_invalid_field(
            with_host_config(|config| config.capture.capture_api = "unknown".to_owned()),
            "capture.capture_api",
        );
    }

    fn with_host_config(mutate: impl FnOnce(&mut HostConfig)) -> Result<HostConfig, ConfigError> {
        let mut config = valid_host_config();
        mutate(&mut config);
        validate_host(config)
    }

    fn assert_invalid_field(result: Result<HostConfig, ConfigError>, field: &str) {
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
