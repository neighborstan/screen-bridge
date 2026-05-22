pub mod config;
pub mod logging;
pub mod net;
pub mod secret;

pub use secret::Secret;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
