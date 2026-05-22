use crate::config::{ConfigError, LoggingConfig};
use crate::logging;
use crate::secret::Secret;

pub const MIN_ACCESS_TOKEN_CHARS: usize = 16;

const PLACEHOLDER_ACCESS_TOKENS: &[&str] = &[
    "change-me-please-16",
    "change-me",
    "changeme",
    "replace-me",
    "default",
    "password",
    "token",
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ConfigWarning {
    AllowSubnetAny,
}

impl ConfigWarning {
    pub fn message(self) -> &'static str {
        match self {
            Self::AllowSubnetAny => {
                "allow_subnet = \"any\" отключает subnet allowlist и должен давать warning"
            }
        }
    }
}

pub(crate) fn validate_access_token(access_token: &Secret) -> Result<(), ConfigError> {
    let value = access_token.as_str().trim();

    if value.is_empty() {
        return invalid("access_token", "значение не должно быть пустым");
    }

    if is_placeholder_access_token(value) {
        return Err(ConfigError::PlaceholderAccessToken);
    }

    if value.chars().count() < MIN_ACCESS_TOKEN_CHARS {
        return invalid(
            "access_token",
            format!("значение должно быть не короче {MIN_ACCESS_TOKEN_CHARS} символов"),
        );
    }

    Ok(())
}

pub(crate) fn validate_logging(config: &LoggingConfig) -> Result<(), ConfigError> {
    validate_non_empty(
        "logging.directory",
        config.directory.as_os_str().to_string_lossy(),
    )?;
    validate_non_empty("logging.file_prefix", &config.file_prefix)?;

    if !logging::is_supported_level(&config.level) {
        return invalid(
            "logging.level",
            "допустимые значения: trace, debug, info, warn, error",
        );
    }

    Ok(())
}

pub(crate) fn validate_non_empty(
    field: &'static str,
    value: impl AsRef<str>,
) -> Result<(), ConfigError> {
    if value.as_ref().trim().is_empty() {
        return invalid(field, "значение не должно быть пустым");
    }

    Ok(())
}

pub(crate) fn validate_port(field: &'static str, port: u16) -> Result<(), ConfigError> {
    if port == 0 {
        return invalid(field, "значение должно быть в диапазоне 1..=65535");
    }

    Ok(())
}

pub(crate) fn validate_positive_u32(field: &'static str, value: u32) -> Result<(), ConfigError> {
    if value == 0 {
        return invalid(field, "значение должно быть больше 0");
    }

    Ok(())
}

pub(crate) fn validate_stream_path(
    field: &'static str,
    stream_path: &str,
) -> Result<(), ConfigError> {
    validate_non_empty(field, stream_path)?;

    if !stream_path.starts_with('/') {
        return invalid(field, "значение должно начинаться с `/`");
    }

    Ok(())
}

pub(crate) fn validate_one_of(
    field: &'static str,
    value: &str,
    allowed: &[&str],
) -> Result<(), ConfigError> {
    if allowed.contains(&value) {
        return Ok(());
    }

    invalid(
        field,
        format!("допустимые значения: {}", allowed.join(", ")),
    )
}

pub(crate) fn invalid<T>(
    field: &'static str,
    message: impl Into<String>,
) -> Result<T, ConfigError> {
    Err(ConfigError::InvalidValue {
        field,
        message: message.into(),
    })
}

fn is_placeholder_access_token(value: &str) -> bool {
    PLACEHOLDER_ACCESS_TOKENS
        .iter()
        .any(|placeholder| value.eq_ignore_ascii_case(placeholder))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_token_should_reject_placeholder_with_actionable_message() {
        // Given
        let token = Secret::new("change-me-please-16");

        // When
        let result = validate_access_token(&token);

        // Then
        let error = result.unwrap_err().to_string();
        assert!(error.contains("измените `access_token`"));
    }

    #[test]
    fn access_token_should_reject_short_value() {
        // Given
        let token = Secret::new("too-short");

        // When
        let result = validate_access_token(&token);

        // Then
        assert!(result.unwrap_err().to_string().contains("не короче 16"));
    }
}
