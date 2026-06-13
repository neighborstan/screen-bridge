//! Запуск viewer playback pipeline и обработка GStreamer bus messages.

use std::fmt;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use gstreamer::prelude::*;
use gstreamer::{self as gst, MessageView};
use screen_bridge_core::config::ViewerConfig;
use screen_bridge_core::net::{self, TcpPreflightConnectKind, TcpPreflightError};

use crate::pipeline::{self, VideoSink, ViewerLaunch};

const TCP_PREFLIGHT_TIMEOUT: Duration = Duration::from_secs(3);

/// Запускает viewer playback по уже загруженному и проверенному config.
pub fn run(config: ViewerConfig) -> Result<()> {
    check_rtsp_tcp_preflight(&config)?;
    gst::init().context("не удалось инициализировать GStreamer")?;

    let (stop_sender, stop_receiver) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = stop_sender.send(());
    })
    .context("не удалось установить Ctrl+C handler")?;

    let primary = pipeline::build_launch(&config, VideoSink::D3d11);
    match run_launch(&primary, &config, &stop_receiver) {
        Ok(()) => Ok(()),
        Err(error) if error.can_retry_with_fallback() => {
            tracing::warn!(
                error = error.to_string(),
                "Primary viewer sink failed; retrying with autovideosink"
            );
            let fallback = pipeline::build_launch(&config, VideoSink::Auto);
            run_launch(&fallback, &config, &stop_receiver).map_err(Into::into)
        }
        Err(error) => Err(error.into()),
    }
}

fn check_rtsp_tcp_preflight(config: &ViewerConfig) -> Result<()> {
    let host = &config.connection.host;
    let port = config.connection.port;

    match net::check_tcp_connect(host, port, TCP_PREFLIGHT_TIMEOUT) {
        Ok(()) => {
            tracing::info!(
                host,
                port,
                "RTSP host TCP preflight succeeded before GStreamer playback"
            );
            Ok(())
        }
        Err(error) => {
            let message = tcp_preflight_failure_message(config, &error);
            tracing::warn!(host, port, error = error.to_string(), message);
            bail!("{message}");
        }
    }
}

fn tcp_preflight_failure_message(config: &ViewerConfig, error: &TcpPreflightError) -> String {
    let host = &config.connection.host;
    let port = config.connection.port;
    let stage = tcp_preflight_failure_stage(error);

    format!(
        "не удалось подключиться к RTSP host по TCP до запуска GStreamer: {host}:{port}. \
         Стадия: {stage}. Причина: {error}. Если host запущен и IP/port указаны верно, проверьте Windows Firewall \
         на host-компьютере: разрешите inbound TCP port {port} для ScreenBridge Host. \
         На viewer-компьютере проверка: Test-NetConnection -ComputerName {host} -Port {port}. \
         В установленной версии используйте shortcut \"ScreenBridge Allow Host Firewall\" на host-компьютере."
    )
}

fn tcp_preflight_failure_stage(error: &TcpPreflightError) -> &'static str {
    match error {
        TcpPreflightError::Resolve { .. } | TcpPreflightError::NoAddresses { .. } => {
            "host/IP не удалось разрешить"
        }
        TcpPreflightError::Connect {
            kind: TcpPreflightConnectKind::Refused,
            ..
        } => "TCP port закрыт или host process не слушает этот адрес",
        TcpPreflightError::Connect {
            kind: TcpPreflightConnectKind::Timeout,
            ..
        } => "TCP timeout, часто это inbound firewall block на host",
        TcpPreflightError::Connect {
            kind: TcpPreflightConnectKind::Unreachable,
            ..
        } => "host/IP недоступен по сети или маршруту",
        TcpPreflightError::Connect {
            kind: TcpPreflightConnectKind::Other,
            ..
        } => "TCP connect failed до RTSP auth и playback",
    }
}

fn run_launch(
    launch: &ViewerLaunch,
    config: &ViewerConfig,
    stop_receiver: &mpsc::Receiver<()>,
) -> Result<(), PlaybackFailure> {
    tracing::info!(
        sink = ?launch.sink(),
        pipeline = launch.sanitized(),
        "Starting ScreenBridge viewer pipeline"
    );

    let element = gst::parse::launch(launch.raw()).map_err(|error| {
        PlaybackFailure::for_sink(
            launch.sink(),
            sanitize_secret(error.to_string(), config),
            PlaybackFailureKind::Playback,
        )
    })?;

    let pipeline = element.dynamic_cast::<gst::Pipeline>().map_err(|_| {
        PlaybackFailure::for_sink(
            launch.sink(),
            "GStreamer launch string не вернул Pipeline".to_owned(),
            PlaybackFailureKind::Playback,
        )
    })?;

    let result = play_pipeline(&pipeline, launch.sink(), config, stop_receiver);
    stop_pipeline(&pipeline);
    result
}

fn play_pipeline(
    pipeline: &gst::Pipeline,
    sink: VideoSink,
    config: &ViewerConfig,
    stop_receiver: &mpsc::Receiver<()>,
) -> Result<(), PlaybackFailure> {
    let bus = pipeline.bus().ok_or_else(|| {
        PlaybackFailure::for_sink(
            sink,
            "GStreamer Pipeline не предоставил bus".to_owned(),
            PlaybackFailureKind::Playback,
        )
    })?;

    pipeline.set_state(gst::State::Playing).map_err(|error| {
        PlaybackFailure::for_sink(
            sink,
            sanitize_secret(error.to_string(), config),
            PlaybackFailureKind::Playback,
        )
    })?;

    println!("ScreenBridge viewer is running. Press Ctrl+C to stop.");

    loop {
        if stop_receiver.try_recv().is_ok() {
            tracing::info!("Ctrl+C received; stopping viewer");
            return Ok(());
        }

        if let Some(message) = bus.timed_pop_filtered(
            gst::ClockTime::from_mseconds(100),
            &[gst::MessageType::Error, gst::MessageType::Eos],
        ) {
            match message.view() {
                MessageView::Eos(..) => {
                    tracing::info!("Viewer pipeline received EOS");
                    return Ok(());
                }
                MessageView::Error(error) => {
                    return Err(playback_failure_from_bus_error(error, sink, config));
                }
                _ => {}
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn playback_failure_from_bus_error(
    error: &gstreamer::message::Error,
    sink: VideoSink,
    config: &ViewerConfig,
) -> PlaybackFailure {
    let source = error
        .src()
        .map(|source| source.path_string())
        .unwrap_or_else(|| "unknown".into());
    let debug = error
        .debug()
        .map(|debug| format!("; debug={debug}"))
        .unwrap_or_default();
    let message = format!(
        "GStreamer error from {source}: {error}{debug}",
        error = error.error()
    );
    let sanitized = sanitize_secret(message, config);
    let kind = classify_failure(&sanitized);

    PlaybackFailure::for_sink(sink, sanitized, kind)
}

fn stop_pipeline(pipeline: &gst::Pipeline) {
    if let Err(error) = pipeline.set_state(gst::State::Null) {
        tracing::warn!(
            error = error.to_string(),
            "Failed to set viewer pipeline to Null"
        );
    }
}

fn sanitize_secret(message: String, config: &ViewerConfig) -> String {
    message.replace(
        config.connection.access_token.as_str(),
        &config.connection.access_token.masked(),
    )
}

fn classify_failure(message: &str) -> PlaybackFailureKind {
    let lower = message.to_ascii_lowercase();

    if lower.contains("unauthorized")
        || lower.contains("not authorized")
        || lower.contains("authentication")
        || lower.contains("401")
    {
        return PlaybackFailureKind::Auth;
    }

    if lower.contains("could not connect")
        || lower.contains("failed to connect")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("service unavailable")
        || lower.contains("503")
    {
        return PlaybackFailureKind::Network;
    }

    PlaybackFailureKind::Playback
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum PlaybackFailureKind {
    Auth,
    Network,
    Playback,
}

#[derive(Debug)]
struct PlaybackFailure {
    sink: VideoSink,
    message: String,
    kind: PlaybackFailureKind,
}

impl PlaybackFailure {
    fn for_sink(sink: VideoSink, message: String, kind: PlaybackFailureKind) -> Self {
        Self {
            sink,
            message,
            kind,
        }
    }

    fn can_retry_with_fallback(&self) -> bool {
        if self.sink != VideoSink::D3d11 || self.kind != PlaybackFailureKind::Playback {
            return false;
        }

        let lower = self.message.to_ascii_lowercase();
        lower.contains("d3d11") || lower.contains("direct3d") || lower.contains("videosink")
    }
}

impl fmt::Display for PlaybackFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.kind {
            PlaybackFailureKind::Auth => "ошибка RTSP authentication",
            PlaybackFailureKind::Network => "ошибка network подключения к RTSP host",
            PlaybackFailureKind::Playback => "ошибка GStreamer playback",
        };

        write!(formatter, "{prefix}: {}", self.message)
    }
}

impl std::error::Error for PlaybackFailure {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_failure_should_detect_auth_errors() {
        // Given
        let message = "GStreamer error: 401 Unauthorized";

        // When
        let result = classify_failure(message);

        // Then
        assert_eq!(result, PlaybackFailureKind::Auth);
    }

    #[test]
    fn classify_failure_should_detect_network_errors() {
        // Given
        let message = "GStreamer error: Could not connect to server";

        // When
        let result = classify_failure(message);

        // Then
        assert_eq!(result, PlaybackFailureKind::Network);
    }

    #[test]
    fn d3d11_playback_error_should_allow_fallback_retry() {
        // Given
        let error = PlaybackFailure::for_sink(
            VideoSink::D3d11,
            "missing element d3d11videosink".to_owned(),
            PlaybackFailureKind::Playback,
        );

        // When
        let result = error.can_retry_with_fallback();

        // Then
        assert!(result);
    }

    #[test]
    fn auth_error_should_not_allow_fallback_retry() {
        // Given
        let error = PlaybackFailure::for_sink(
            VideoSink::D3d11,
            "401 Unauthorized".to_owned(),
            PlaybackFailureKind::Auth,
        );

        // When
        let result = error.can_retry_with_fallback();

        // Then
        assert!(!result);
    }

    #[test]
    fn tcp_preflight_failure_message_should_include_firewall_next_action() {
        // Given
        let mut config = ViewerConfig::default();
        config.connection.host = "192.168.1.139".to_owned();
        config.connection.port = 8554;
        let error = TcpPreflightError::Connect {
            host: config.connection.host.clone(),
            port: config.connection.port,
            timeout_ms: 3000,
            kind: TcpPreflightConnectKind::Timeout,
            attempts: "192.168.1.139:8554: timed out".to_owned(),
        };

        // When
        let message = tcp_preflight_failure_message(&config, &error);

        // Then
        assert!(message.contains("192.168.1.139:8554"));
        assert!(message.contains("TCP timeout"));
        assert!(message.contains("Windows Firewall"));
        assert!(message.contains("Test-NetConnection -ComputerName 192.168.1.139 -Port 8554"));
        assert!(message.contains("ScreenBridge Allow Host Firewall"));
    }
}
