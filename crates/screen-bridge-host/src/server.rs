//! RTSP server lifecycle для host pipeline.

use std::net::Ipv4Addr;

use anyhow::{Context, Result};
use gstreamer_rtsp_server::gst_rtsp;
use gstreamer_rtsp_server::prelude::*;
use gstreamer_rtsp_server::{glib, RTSPMediaFactory, RTSPServer};
use screen_bridge_core::config::ServerConfig;

pub(crate) struct HostServer {
    bind_ip: Ipv4Addr,
    port: u16,
    stream_path: String,
    main_loop: glib::MainLoop,
    source_id: glib::SourceId,
}

impl HostServer {
    pub(crate) fn start(bind_ip: Ipv4Addr, config: &ServerConfig, launch: &str) -> Result<Self> {
        let main_loop = glib::MainLoop::new(None, false);
        let server = RTSPServer::new();
        server.set_address(&bind_ip.to_string());
        server.set_service(&config.port.to_string());

        let mounts = server
            .mount_points()
            .context("не удалось получить RTSP mount points")?;

        let factory = RTSPMediaFactory::new();
        factory.set_launch(launch);
        factory.set_shared(true);
        factory.set_stop_on_disconnect(true);
        factory.set_protocols(gst_rtsp::RTSPLowerTrans::TCP);
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
