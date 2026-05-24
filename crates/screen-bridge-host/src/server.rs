//! RTSP server lifecycle для host pipeline.

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use gstreamer_rtsp_server::gst_rtsp;
use gstreamer_rtsp_server::prelude::*;
use gstreamer_rtsp_server::{
    glib, RTSPAuth, RTSPClient, RTSPFilterResult, RTSPMediaFactory, RTSPServer, RTSPSession,
    RTSPSessionPool,
};
use screen_bridge_core::config::{SecurityConfig, ServerConfig};

use crate::auth;
use crate::peer_ip::{self, PeerAddress};
use crate::session_limit::{self, ActiveClientLimit};
use crate::subnet_guard::{SubnetDecision, SubnetGuard};

pub(crate) struct HostServer {
    bind_ip: Ipv4Addr,
    port: u16,
    stream_path: String,
    main_loop: glib::MainLoop,
    source_id: glib::SourceId,
}

impl HostServer {
    pub(crate) fn start(
        bind_ip: Ipv4Addr,
        config: &ServerConfig,
        security: &SecurityConfig,
        subnet_guard: &SubnetGuard,
        launch: &str,
    ) -> Result<Self> {
        let main_loop = glib::MainLoop::new(None, false);
        let server = RTSPServer::new();
        server.set_address(&bind_ip.to_string());
        server.set_service(&config.port.to_string());

        let auth = RTSPAuth::new();
        auth::configure_auth(&auth, security);
        server.set_auth(Some(&auth));

        let session_pool = session_limit::build_session_pool();
        server.set_session_pool(Some(&session_pool));

        let active_client_limit = ActiveClientLimit::new(config.max_clients);
        install_client_hooks(&server, subnet_guard, &session_pool, active_client_limit);

        let mounts = server
            .mount_points()
            .context("не удалось получить RTSP mount points")?;

        let factory = RTSPMediaFactory::new();
        factory.set_launch(launch);
        factory.set_shared(true);
        factory.set_eos_shutdown(true);
        factory.set_stop_on_disconnect(true);
        factory.set_protocols(gst_rtsp::RTSPLowerTrans::TCP);
        auth::require_authentication(&factory);
        mounts.add_factory(&config.stream_path, factory);

        let source_id = server
            .attach(None)
            .context("не удалось запустить RTSP server")?;

        Ok(Self {
            bind_ip,
            port: config.port,
            stream_path: config.stream_path.clone(),
            main_loop,
            source_id,
        })
    }

    pub(crate) fn bind_ip(&self) -> Ipv4Addr {
        self.bind_ip
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn stream_path(&self) -> &str {
        &self.stream_path
    }

    pub(crate) fn run_until_ctrl_c(self) -> Result<()> {
        let stop_loop = self.main_loop.clone();
        ctrlc::set_handler(move || {
            println!("Shutting down ScreenBridge host...");
            tracing::info!("Ctrl+C received; shutting down host");
            stop_loop.quit();
        })
        .context("не удалось установить Ctrl+C handler")?;

        self.main_loop.run();
        self.source_id.remove();
        tracing::info!("ScreenBridge host stopped");
        Ok(())
    }
}

fn install_client_hooks(
    server: &RTSPServer,
    subnet_guard: &SubnetGuard,
    session_pool: &RTSPSessionPool,
    active_client_limit: ActiveClientLimit,
) {
    let subnet_guard = subnet_guard.clone();
    let session_pool = session_pool.clone();
    server.connect_client_connected(move |_server, client| {
        let client_has_slot = Arc::new(AtomicBool::new(false));
        let client_sessions = Arc::new(Mutex::new(Vec::new()));

        let peer_address = peer_ip::client_peer_address(client);
        match subnet_guard.check_peer(&peer_address) {
            SubnetDecision::Allow => {
                tracing::info!(peer = %peer_address, "RTSP client connected");
                subnet_guard.log_client_warning_if_any(&peer_address);
            }
            SubnetDecision::Reject { ref reason } => {
                tracing::warn!(
                    peer = %peer_address,
                    reason = reason.as_str(),
                    "RTSP client will be rejected by subnet guard"
                );
            }
        }

        install_subnet_request_hooks(client, &subnet_guard, peer_address.clone());

        let active_limit_for_setup = active_client_limit.clone();
        let subnet_guard_for_setup = subnet_guard.clone();
        let peer_address_for_setup = peer_address.clone();
        let client_has_slot_for_setup = Arc::clone(&client_has_slot);
        client.connect_pre_setup_request(move |_client, context| {
            if let Some(status) =
                subnet_rejection_status(&subnet_guard_for_setup, &peer_address_for_setup, "SETUP")
            {
                return status;
            }

            let has_existing_session = context.session().is_some();

            if has_existing_session || client_has_slot_for_setup.load(Ordering::Acquire) {
                return gst_rtsp::RTSPStatusCode::Ok;
            }

            if !active_limit_for_setup.try_acquire() {
                tracing::warn!(
                    active_clients = active_limit_for_setup.active_clients(),
                    max_clients = active_limit_for_setup.max_clients(),
                    "RTSP client rejected; reason=max_clients reached"
                );
                return gst_rtsp::RTSPStatusCode::ServiceUnavailable;
            }

            client_has_slot_for_setup.store(true, Ordering::Release);
            gst_rtsp::RTSPStatusCode::Ok
        });

        let client_sessions_for_new_session = Arc::clone(&client_sessions);
        client.connect_new_session(move |_client, session| {
            if let Ok(mut sessions) = client_sessions_for_new_session.lock() {
                sessions.push(session.clone());
            }
        });

        let session_pool_for_close = session_pool.clone();
        let active_limit_for_close = active_client_limit.clone();
        let client_has_slot_for_close = Arc::clone(&client_has_slot);
        let client_sessions_for_close = Arc::clone(&client_sessions);
        client.connect_closed(move |client| {
            if client_has_slot_for_close.swap(false, Ordering::AcqRel) {
                active_limit_for_close.release();
            }

            let removed_client_sessions =
                remove_client_sessions(client, &session_pool_for_close, &client_sessions_for_close);
            let removed_expired_sessions = session_pool_for_close.cleanup();
            tracing::info!("RTSP client closed");
            if removed_client_sessions > 0 || removed_expired_sessions > 0 {
                tracing::info!(
                    removed_client_sessions,
                    removed_expired_sessions,
                    "RTSP session pool cleaned up"
                );
            }
        });
    });
}

fn remove_client_sessions(
    client: &RTSPClient,
    session_pool: &RTSPSessionPool,
    client_sessions: &Arc<Mutex<Vec<RTSPSession>>>,
) -> usize {
    let sessions = match client_sessions.lock() {
        Ok(mut sessions) => std::mem::take(&mut *sessions),
        Err(_) => client_sessions_from_filter(client),
    };
    let mut removed_sessions = 0;

    for session in sessions {
        match session_pool.remove(&session) {
            Ok(()) => removed_sessions += 1,
            Err(error) => tracing::debug!(
                error = error.to_string(),
                "RTSP session was already removed from pool"
            ),
        }
    }

    removed_sessions
}

fn client_sessions_from_filter(client: &RTSPClient) -> Vec<RTSPSession> {
    let mut ref_session = ref_session_from_client;
    client.session_filter(Some(&mut ref_session))
}

fn ref_session_from_client(_client: &RTSPClient, _session: &RTSPSession) -> RTSPFilterResult {
    RTSPFilterResult::Ref
}

fn install_subnet_request_hooks(
    client: &gstreamer_rtsp_server::RTSPClient,
    subnet_guard: &SubnetGuard,
    peer_address: PeerAddress,
) {
    let options_guard = subnet_guard.clone();
    let options_peer = peer_address.clone();
    client.connect_pre_options_request(move |_client, _context| {
        subnet_rejection_status(&options_guard, &options_peer, "OPTIONS")
            .unwrap_or(gst_rtsp::RTSPStatusCode::Ok)
    });

    let describe_guard = subnet_guard.clone();
    let describe_peer = peer_address.clone();
    client.connect_pre_describe_request(move |_client, _context| {
        subnet_rejection_status(&describe_guard, &describe_peer, "DESCRIBE")
            .unwrap_or(gst_rtsp::RTSPStatusCode::Ok)
    });

    let play_guard = subnet_guard.clone();
    let play_peer = peer_address.clone();
    client.connect_pre_play_request(move |_client, _context| {
        subnet_rejection_status(&play_guard, &play_peer, "PLAY")
            .unwrap_or(gst_rtsp::RTSPStatusCode::Ok)
    });
}

fn subnet_rejection_status(
    subnet_guard: &SubnetGuard,
    peer_address: &PeerAddress,
    method: &str,
) -> Option<gst_rtsp::RTSPStatusCode> {
    match subnet_guard.check_peer(peer_address) {
        SubnetDecision::Allow => None,
        SubnetDecision::Reject { reason } => {
            tracing::warn!(
                peer = %peer_address,
                reason = reason.as_str(),
                method,
                "RTSP request rejected by subnet guard"
            );
            Some(gst_rtsp::RTSPStatusCode::Forbidden)
        }
    }
}
