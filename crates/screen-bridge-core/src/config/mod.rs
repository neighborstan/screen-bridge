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
pub enum ConfigError {
    #[error("не удалось прочитать config `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("не удалось разобрать TOML config: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("неверное значение `{field}`: {message}")]
    InvalidValue {
        field: &'static str,
        message: String,
    },
    #[error("измените `access_token` в config: placeholder/default token недопустим")]
    PlaceholderAccessToken,
    #[error("неверное значение `security.allow_subnet` `{value}`: {source}")]
    InvalidSubnet {
        value: String,
        #[source]
        source: crate::net::SubnetParseError,
    },
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LoggingConfig {
    pub level: String,
    pub directory: PathBuf,
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

pub fn parse_host_toml(input: &str) -> Result<HostConfig, ConfigError> {
    toml::from_str(input).map_err(ConfigError::Toml)
}

pub fn parse_viewer_toml(input: &str) -> Result<ViewerConfig, ConfigError> {
    toml::from_str(input).map_err(ConfigError::Toml)
}

pub fn load_host(path: impl AsRef<Path>) -> Result<HostConfig, ConfigError> {
    let path = path.as_ref();
    let content = read_config(path)?;
    let config = parse_host_toml(&content)?;

    validate_host(config)
}

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
