//! RTSP server lifecycle для host pipeline.

use std::net::Ipv4Addr;

use anyhow::{Context, Result};
use gstreamer_rtsp_server::gst_rtsp;
use gstreamer_rtsp_server::prelude::*;
use gstreamer_rtsp_server::{glib, RTSPAuth, RTSPMediaFactory, RTSPServer, RTSPSessionPool};
use screen_bridge_core::config::{SecurityConfig, ServerConfig};

use crate::auth;
use crate::peer_ip::{self, PeerAddress};
use crate::session_limit;
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

        let session_pool = session_limit::build_session_pool(config);
        server.set_session_pool(Some(&session_pool));

        install_client_hooks(&server, subnet_guard, &session_pool, config.max_clients);

        let mounts = server
            .mount_points()
            .context("не удалось получить RTSP mount points")?;

        let factory = RTSPMediaFactory::new();
        factory.set_launch(launch);
        factory.set_shared(true);
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

    pub(crate) fn rtsp_url(&self) -> String {
        format!("rtsp://{}:{}{}", self.bind_ip, self.port, self.stream_path)
    }

    pub(crate) fn run_until_ctrl_c(self) -> Result<()> {
        let stop_loop = self.main_loop.clone();
        ctrlc::set_handler(move || {
            stop_loop.quit();
        })
        .context("не удалось установить Ctrl+C handler")?;

        self.main_loop.run();
        self.source_id.remove();
        Ok(())
    }
}

fn install_client_hooks(
    server: &RTSPServer,
    subnet_guard: &SubnetGuard,
    session_pool: &RTSPSessionPool,
    max_clients: u16,
) {
    let subnet_guard = subnet_guard.clone();
    let session_pool = session_pool.clone();
    server.connect_client_connected(move |_server, client| {
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

        let session_pool_for_setup = session_pool.clone();
        let subnet_guard_for_setup = subnet_guard.clone();
        let peer_address_for_setup = peer_address.clone();
        client.connect_pre_setup_request(move |_client, context| {
            if let Some(status) =
                subnet_rejection_status(&subnet_guard_for_setup, &peer_address_for_setup, "SETUP")
            {
                return status;
            }

            let active_sessions = session_pool_for_setup.n_sessions();
            let has_existing_session = context.session().is_some();

            if session_limit::should_reject_new_session(
                active_sessions,
                max_clients,
                has_existing_session,
            ) {
                tracing::warn!(
                    active_sessions,
                    max_clients,
                    "RTSP client rejected; reason=max_clients reached"
                );
                return gst_rtsp::RTSPStatusCode::ServiceUnavailable;
            }

            gst_rtsp::RTSPStatusCode::Ok
        });

        client.connect_closed(move |_| {
            tracing::info!("RTSP client closed");
        });
    });
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
