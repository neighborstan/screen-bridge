//! Ограничение числа RTSP sessions для MVP.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use gstreamer_rtsp_server::prelude::*;
use gstreamer_rtsp_server::RTSPSessionPool;
pub(crate) fn build_session_pool() -> RTSPSessionPool {
    let pool = RTSPSessionPool::new();
    // Stale RTSP sessions can survive an abrupt client disconnect long enough
    // to block immediate VLC reconnects. The actual MVP limit is enforced by
    // ActiveClientLimit below, while the pool remains responsible for session
    // storage and cleanup.
    pool.set_max_sessions(0);
    pool
}

#[derive(Debug, Clone)]
pub(crate) struct ActiveClientLimit {
    active_clients: Arc<AtomicUsize>,
    max_clients: usize,
}

impl ActiveClientLimit {
    pub(crate) fn new(max_clients: u16) -> Self {
        Self {
            active_clients: Arc::new(AtomicUsize::new(0)),
            max_clients: usize::from(max_clients),
        }
    }

    pub(crate) fn try_acquire(&self) -> bool {
        let mut current = self.active_clients.load(Ordering::Acquire);

        loop {
            if current >= self.max_clients {
                return false;
            }

            match self.active_clients.compare_exchange(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }

    pub(crate) fn release(&self) {
        let _ = self
            .active_clients
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_sub(1)
            });
    }

    pub(crate) fn active_clients(&self) -> usize {
        self.active_clients.load(Ordering::Acquire)
    }

    pub(crate) fn max_clients(&self) -> usize {
        self.max_clients
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_pool_should_not_cap_stale_sessions() {
        // Given
        gstreamer::init().unwrap();
        // When
        let pool = build_session_pool();

        // Then
        assert_eq!(pool.max_sessions(), 0);
    }

    #[test]
    fn active_client_limit_should_reject_when_limit_is_reached() {
        // Given
        let limit = ActiveClientLimit::new(1);

        // When / Then
        assert!(limit.try_acquire());
        assert!(!limit.try_acquire());
        assert_eq!(limit.active_clients(), 1);
        limit.release();
        assert_eq!(limit.active_clients(), 0);
        assert!(limit.try_acquire());
    }
}
