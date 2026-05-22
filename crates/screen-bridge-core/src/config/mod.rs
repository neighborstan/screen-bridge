//! Конфиги host и viewer.
//!
//! `parse_*_toml` только читает TOML в структуры. `load_*` читает файл и
//! применяет validation, поэтому example configs с placeholder token не
//! подходят для запуска без ручной правки.

mod host;
mod validate;
mod viewer;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

pub use host::{
    validate_host, CaptureConfig, HostConfig, SecurityConfig, ServerConfig, VideoConfig,
};
pub use validate::{ConfigWarning, MIN_ACCESS_TOKEN_CHARS};
pub use viewer::{validate_viewer, ConnectionConfig, PlaybackConfig, ViewerConfig};

#[derive(Debug, Error)]
/// Ошибки чтения, разбора и проверки config files.
pub enum ConfigError {
    /// Config file не удалось прочитать с диска.
    #[error("не удалось прочитать config `{path}`: {source}")]
    Read {
        /// Путь к config file.
        path: PathBuf,
        /// Исходная ошибка чтения файла.
        #[source]
        source: io::Error,
    },
    /// TOML синтаксически неверный или не совпадает со schema.
    #[error("не удалось разобрать TOML config: {0}")]
    Toml(#[from] toml::de::Error),
    /// Значение прошло TOML parsing, но нарушает правила MVP.
    #[error("неверное значение `{field}`: {message}")]
    InvalidValue {
        /// Имя поля в config.
        field: &'static str,
        /// Понятное описание ошибки для пользователя.
        message: String,
    },
    /// Token остался известным placeholder value.
    #[error("измените `access_token` в config: placeholder/default token недопустим")]
    PlaceholderAccessToken,
    /// `allow_subnet` не является IPv4 CIDR или значением "any".
    #[error("неверное значение `security.allow_subnet` `{value}`: {source}")]
    InvalidSubnet {
        /// Значение из config.
        value: String,
        /// Ошибка разбора subnet.
        #[source]
        source: crate::net::SubnetParseError,
    },
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
/// Общие настройки логирования для host и viewer.
pub struct LoggingConfig {
    /// Минимальный уровень логов.
    pub level: String,
    /// Директория для файлов логов.
    pub directory: PathBuf,
    /// Префикс имени файла лога.
    pub file_prefix: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_owned(),
            directory: PathBuf::from("logs"),
            file_prefix: "screen-bridge".to_owned(),
        }
    }
}

/// Разбирает host TOML без validation.
pub fn parse_host_toml(input: &str) -> Result<HostConfig, ConfigError> {
    toml::from_str(input).map_err(ConfigError::Toml)
}

/// Разбирает viewer TOML без validation.
pub fn parse_viewer_toml(input: &str) -> Result<ViewerConfig, ConfigError> {
    toml::from_str(input).map_err(ConfigError::Toml)
}

/// Читает host config из файла и проверяет его по правилам MVP.
pub fn load_host(path: impl AsRef<Path>) -> Result<HostConfig, ConfigError> {
    let path = path.as_ref();
    let content = read_config(path)?;
    let config = parse_host_toml(&content)?;

    validate_host(config)
}

/// Читает viewer config из файла и проверяет его по правилам MVP.
pub fn load_viewer(path: impl AsRef<Path>) -> Result<ViewerConfig, ConfigError> {
    let path = path.as_ref();
    let content = read_config(path)?;
    let config = parse_viewer_toml(&content)?;

    validate_viewer(config)
}

fn read_config(path: &Path) -> Result<String, ConfigError> {
    fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })
}
