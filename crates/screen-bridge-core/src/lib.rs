//! Общая библиотека ScreenBridge.
//!
//! Здесь лежит код, который нужен и host, и viewer: конфиги, логирование,
//! маскирование secret values и работа с локальной сетью.
#![warn(missing_docs)]

/// Чтение и проверка TOML-конфигов.
pub mod config;

/// Настройка логов для stdout и файлов.
pub mod logging;

/// Помощники для локальных IPv4 и subnet allowlist.
pub mod net;

/// Настройка runtime окружения для app-local installer layout.
pub mod runtime;

/// Безопасное отображение secret values.
pub mod secret;

pub use secret::Secret;

/// Версия crate из `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Возвращает версию core crate.
pub fn version() -> &'static str {
    VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_should_match_package_version() {
        // Given
        let package_version = env!("CARGO_PKG_VERSION");

        // When
        let result = version();

        // Then
        assert_eq!(result, package_version);
    }
}
