//! Генерация GStreamer launch string для viewer playback.
//!
//! Raw launch string содержит настоящий token и предназначен только для
//! GStreamer. Sanitized launch string можно писать в logs и snapshots.

use screen_bridge_core::config::ViewerConfig;

/// Вариант video sink для viewer pipeline.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VideoSink {
    /// Primary Windows sink через Direct3D 11.
    D3d11,
    /// Fallback sink с автоматическим выбором доступного video sink.
    Auto,
}

impl VideoSink {
    fn description(self) -> &'static str {
        match self {
            Self::D3d11 => "d3d11videosink sync=false",
            Self::Auto => "videoconvert ! autovideosink sync=false",
        }
    }
}

/// Raw и sanitized варианты одного viewer launch string.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ViewerLaunch {
    raw: String,
    sanitized: String,
    sink: VideoSink,
}

impl ViewerLaunch {
    /// Возвращает строку для `gst_parse_launch`.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Возвращает строку, безопасную для логов и diagnostic output.
    pub fn sanitized(&self) -> &str {
        &self.sanitized
    }

    /// Возвращает video sink, для которого построена строка.
    pub fn sink(&self) -> VideoSink {
        self.sink
    }
}

/// Строит viewer playback pipeline для выбранного video sink.
pub fn build_launch(config: &ViewerConfig, sink: VideoSink) -> ViewerLaunch {
    let raw = build_launch_string(
        config,
        sink,
        config.connection.access_token.as_str().to_owned(),
    );
    let sanitized = build_launch_string(config, sink, config.connection.access_token.masked());

    ViewerLaunch {
        raw,
        sanitized,
        sink,
    }
}

fn build_launch_string(config: &ViewerConfig, sink: VideoSink, token: String) -> String {
    format!(
        "rtspsrc location={location} protocols=tcp latency={latency} user-id={user} user-pw={token} \
         ! rtph264depay \
         ! h264parse \
         ! decodebin \
         ! {sink}",
        location = quote_launch_value(&rtsp_location(config)),
        latency = config.playback.latency_ms,
        user = quote_launch_value(&config.connection.auth_user),
        token = quote_launch_value(&token),
        sink = sink.description(),
    )
}

fn rtsp_location(config: &ViewerConfig) -> String {
    format!(
        "rtsp://{}:{}{}",
        config.connection.host, config.connection.port, config.connection.stream_path
    )
}

fn quote_launch_value(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');

    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            _ => quoted.push(character),
        }
    }

    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use screen_bridge_core::config::ViewerConfig;
    use screen_bridge_core::Secret;

    use super::*;

    fn viewer_config() -> ViewerConfig {
        let mut config = ViewerConfig::default();
        config.connection.host = "192.168.1.151".to_owned();
        config.connection.access_token = Secret::new("valid-token-1234");
        config
    }

    #[test]
    fn launch_string_should_include_rtsp_tcp_auth_and_h264_chain() {
        // Given
        let config = viewer_config();

        // When
        let launch = build_launch(&config, VideoSink::D3d11);

        // Then
        assert!(launch.raw().contains("rtspsrc"));
        assert!(launch
            .raw()
            .contains("location=\"rtsp://192.168.1.151:8554/screen\""));
        assert!(launch.raw().contains("protocols=tcp"));
        assert!(launch.raw().contains("latency=100"));
        assert!(launch.raw().contains("user-id=\"viewer\""));
        assert!(launch.raw().contains("user-pw=\"valid-token-1234\""));
        assert!(launch.raw().contains("rtph264depay"));
        assert!(launch.raw().contains("h264parse"));
        assert!(launch.raw().contains("decodebin"));
        assert!(launch.raw().contains("d3d11videosink sync=false"));
    }

    #[test]
    fn sanitized_launch_string_should_mask_token() {
        // Given
        let config = viewer_config();

        // When
        let launch = build_launch(&config, VideoSink::D3d11);

        // Then
        assert!(launch.sanitized().contains("user-pw=\"val**********234\""));
        assert!(!launch
            .sanitized()
            .contains(config.connection.access_token.as_str()));
    }

    #[test]
    fn fallback_launch_string_should_use_autovideosink() {
        // Given
        let config = viewer_config();

        // When
        let launch = build_launch(&config, VideoSink::Auto);

        // Then
        assert_eq!(launch.sink(), VideoSink::Auto);
        assert!(launch
            .raw()
            .contains("videoconvert ! autovideosink sync=false"));
        assert!(!launch.raw().contains("d3d11videosink"));
    }

    #[test]
    fn launch_string_should_escape_quoted_values() {
        // Given
        let mut config = viewer_config();
        config.connection.auth_user = "view\"er".to_owned();
        config.connection.access_token = Secret::new("token-with-quote\"1234");

        // When
        let launch = build_launch(&config, VideoSink::D3d11);

        // Then
        assert!(launch.raw().contains("user-id=\"view\\\"er\""));
        assert!(launch
            .raw()
            .contains("user-pw=\"token-with-quote\\\"1234\""));
    }
}
