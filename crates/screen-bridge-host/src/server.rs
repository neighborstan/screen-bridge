//! RTSP server lifecycle для host pipeline.

use std::net::Ipv4Addr;

use anyhow::{Context, Result};
use gstreamer_rtsp_server::gst_rtsp;
use gstreamer_rtsp_server::prelude::*;
use gstreamer_rtsp_server::{glib, RTSPAuth, RTSPMediaFactory, RTSPServer, RTSPSessionPool};
use screen_bridge_core::config::{SecurityConfig, ServerConfig};

use crate::auth;
use crate::session_limit;
use crate::subnet_guard::SubnetGuard;

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
        tracing::info!(
            "RTSP client connected; peer IP is unavailable through safe gstreamer-rtsp-server bindings"
        );
        subnet_guard.log_client_warning_if_any();

        let session_pool_for_setup = session_pool.clone();
        client.connect_pre_setup_request(move |_client, context| {
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
