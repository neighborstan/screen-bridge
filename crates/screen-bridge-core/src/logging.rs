//! Инициализация `tracing` для консоли и файлов.

use std::fs;

use thiserror::Error;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, Layer};

use crate::config::LoggingConfig;

#[derive(Debug, Error)]
/// Ошибки настройки логирования.
pub enum LoggingError {
    /// Уровень логов не входит в поддержанный список.
    #[error("неверное значение logging.level `{0}`")]
    InvalidLevel(String),
    /// Директорию для логов не удалось создать.
    #[error("не удалось создать директорию логов: {0}")]
    CreateDirectory(#[from] std::io::Error),
    /// Глобальный tracing subscriber уже был установлен или не принял config.
    #[error("не удалось инициализировать logging: {0}")]
    Init(String),
}

/// Guard, который держит file appender живым до завершения процесса.
pub struct LoggingGuard {
    _file_guard: WorkerGuard,
}

/// Настраивает вывод логов в stdout и ежедневный файл.
pub fn init(config: &LoggingConfig) -> Result<LoggingGuard, LoggingError> {
    let level_filter = parse_level_filter(&config.level)?;
    fs::create_dir_all(&config.directory)?;

    let file_name_prefix = format!("{}.log", config.file_prefix);
    let file_appender = tracing_appender::rolling::daily(&config.directory, file_name_prefix);
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_filter(level_filter);
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(file_writer)
        .with_filter(level_filter);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .try_init()
        .map_err(|error| LoggingError::Init(error.to_string()))?;

    Ok(LoggingGuard {
        _file_guard: file_guard,
    })
}

/// Проверяет, поддержан ли уровень логов из config.
pub fn is_supported_level(level: &str) -> bool {
    parse_level_filter(level).is_ok()
}

/// Преобразует строковый уровень логов в фильтр `tracing`.
pub fn parse_level_filter(level: &str) -> Result<LevelFilter, LoggingError> {
    match level.trim().to_ascii_lowercase().as_str() {
        "trace" => Ok(LevelFilter::TRACE),
        "debug" => Ok(LevelFilter::DEBUG),
        "info" => Ok(LevelFilter::INFO),
        "warn" => Ok(LevelFilter::WARN),
        "error" => Ok(LevelFilter::ERROR),
        _ => Err(LoggingError::InvalidLevel(level.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_level_filter_should_accept_known_levels() {
        // Given
        let cases = [
            ("trace", LevelFilter::TRACE),
            ("debug", LevelFilter::DEBUG),
            ("info", LevelFilter::INFO),
            ("warn", LevelFilter::WARN),
            ("error", LevelFilter::ERROR),
        ];

        for (input, expected) in cases {
            // When
            let result = parse_level_filter(input).unwrap();

            // Then
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn parse_level_filter_should_reject_unknown_level() {
        // Given
        let level = "verbose";

        // When
        let result = parse_level_filter(level);

        // Then
        assert!(result.is_err());
    }
}
