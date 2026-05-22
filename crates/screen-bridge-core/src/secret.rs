//! Типы и функции для безопасного отображения secret values.

use std::fmt;

use serde::Deserialize;

const VISIBLE_PREFIX_CHARS: usize = 3;
const VISIBLE_SUFFIX_CHARS: usize = 3;
const MAX_MASK_CHARS: usize = 11;

#[derive(Clone, Deserialize, Eq, Hash, PartialEq)]
#[serde(transparent)]
/// Secret value, который нельзя случайно вывести целиком через `Display` или
/// `Debug`.
pub struct Secret(String);

impl Secret {
    /// Создает secret value из строки.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Возвращает исходное значение для кода, которому нужен настоящий token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Возвращает замаскированное значение для логов и diagnostic output.
    pub fn masked(&self) -> String {
        mask_token(&self.0)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Secret")
            .field(&self.masked())
            .finish()
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.masked())
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Маскирует token по единому правилу проекта.
pub fn mask_token(token: &str) -> String {
    let chars = token.chars().collect::<Vec<_>>();
    let len = chars.len();

    if len == 0 {
        return "***".to_owned();
    }

    if len <= VISIBLE_PREFIX_CHARS + VISIBLE_SUFFIX_CHARS {
        return "*".repeat(len);
    }

    let prefix = chars.iter().take(VISIBLE_PREFIX_CHARS).collect::<String>();
    let suffix = chars
        .iter()
        .skip(len - VISIBLE_SUFFIX_CHARS)
        .collect::<String>();
    let mask_len = (len - VISIBLE_PREFIX_CHARS - VISIBLE_SUFFIX_CHARS).min(MAX_MASK_CHARS);

    format!("{prefix}{}{suffix}", "*".repeat(mask_len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_token_should_match_project_example() {
        // Given
        let token = "change-me-please-16";

        // When
        let result = mask_token(token);

        // Then
        assert_eq!(result, "cha***********-16");
    }

    #[test]
    fn display_should_not_expose_full_token() {
        // Given
        let token = Secret::new("valid-token-1234");

        // When
        let result = token.to_string();

        // Then
        assert!(!result.contains(token.as_str()));
        assert_eq!(result, "val**********234");
    }

    #[test]
    fn debug_should_not_expose_full_token() {
        // Given
        let token = Secret::new("valid-token-1234");

        // When
        let result = format!("{token:?}");

        // Then
        assert!(!result.contains(token.as_str()));
        assert!(result.contains("Secret"));
    }

    #[test]
    fn short_token_should_be_fully_masked() {
        // Given
        let token = "short";

        // When
        let result = mask_token(token);

        // Then
        assert_eq!(result, "*****");
    }
}
