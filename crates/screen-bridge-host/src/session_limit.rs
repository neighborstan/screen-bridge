//! Ограничение числа RTSP sessions для MVP.

use gstreamer_rtsp_server::prelude::*;
use gstreamer_rtsp_server::RTSPSessionPool;
use screen_bridge_core::config::ServerConfig;

pub(crate) fn build_session_pool(config: &ServerConfig) -> RTSPSessionPool {
    let pool = RTSPSessionPool::new();
    // Глобальный лимит sessions на server. Для MVP с одним stream path это
    // эквивалентно одному viewer; при нескольких stream path нужно пересмотреть.
    pool.set_max_sessions(u32::from(config.max_clients));
    pool
}

pub(crate) fn should_reject_new_session(
    active_sessions: u32,
    max_clients: u16,
    has_existing_session: bool,
) -> bool {
    !has_existing_session && active_sessions >= u32::from(max_clients)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_pool_should_use_configured_max_clients() {
        // Given
        gstreamer::init().unwrap();
        let config = ServerConfig::default();

        // When
        let pool = build_session_pool(&config);

        // Then
        assert_eq!(pool.max_sessions(), 1);
    }

    #[test]
    fn should_reject_new_session_when_limit_is_reached() {
        // Given / When / Then
        assert!(should_reject_new_session(1, 1, false));
        assert!(!should_reject_new_session(0, 1, false));
        assert!(!should_reject_new_session(1, 1, true));
    }
}
