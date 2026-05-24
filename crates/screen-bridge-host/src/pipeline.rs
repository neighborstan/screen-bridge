//! Генерация GStreamer launch string для host stream.
//!
//! Строки держатся отдельно от RTSP server, чтобы проверять их unit-тестами
//! без запуска реального GStreamer pipeline.

use anyhow::{bail, Result};
use screen_bridge_core::config::{CaptureConfig, VideoConfig};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Encoder {
    MediaFoundationH264,
    NvidiaD3d11H264,
    NvidiaAutoGpuH264,
    IntelQsvH264,
    AmdAmfH264,
    X264,
}

impl Encoder {
    fn element_name(self) -> &'static str {
        match self {
            Self::MediaFoundationH264 => "mfh264enc",
            Self::NvidiaD3d11H264 => "nvd3d11h264enc",
            Self::NvidiaAutoGpuH264 => "nvautogpuh264enc",
            Self::IntelQsvH264 => "qsvh264enc",
            Self::AmdAmfH264 => "amfh264enc",
            Self::X264 => "x264enc",
        }
    }
}

pub(crate) trait ElementAvailability {
    fn is_available(&self, element: &str) -> bool;
}

pub(crate) struct GstElementAvailability;

impl ElementAvailability for GstElementAvailability {
    fn is_available(&self, element: &str) -> bool {
        gstreamer::ElementFactory::find(element).is_some()
    }
}

pub(crate) fn build_launch_string(
    video: &VideoConfig,
    capture: &CaptureConfig,
    availability: &impl ElementAvailability,
) -> Result<String> {
    let encoder = select_encoder(video, availability)?;

    match encoder {
        Encoder::MediaFoundationH264 => Ok(build_mfh264_launch_string(video, capture)),
        Encoder::NvidiaD3d11H264 | Encoder::NvidiaAutoGpuH264 => {
            Ok(build_nvenc_launch_string(video, capture, encoder))
        }
        Encoder::IntelQsvH264 => Ok(build_qsv_launch_string(video, capture)),
        Encoder::AmdAmfH264 => Ok(build_amf_launch_string(video, capture)),
        Encoder::X264 => Ok(build_x264_launch_string(video, capture)),
    }
}

fn select_encoder(video: &VideoConfig, availability: &impl ElementAvailability) -> Result<Encoder> {
    match video.encoder.as_str() {
        "auto" => {
            for encoder in [
                Encoder::MediaFoundationH264,
                Encoder::NvidiaD3d11H264,
                Encoder::NvidiaAutoGpuH264,
                Encoder::IntelQsvH264,
                Encoder::AmdAmfH264,
                Encoder::X264,
            ] {
                if availability.is_available(encoder.element_name()) {
                    return Ok(encoder);
                }
            }

            bail!(
                "не найден H.264 encoder: требуются mfh264enc, nvd3d11h264enc, \
                 nvautogpuh264enc, qsvh264enc, amfh264enc или x264enc"
            )
        }
        "software_only" => {
            if availability.is_available(Encoder::X264.element_name()) {
                return Ok(Encoder::X264);
            }

            bail!("video.encoder = \"software_only\", но GStreamer element x264enc не найден")
        }
        other => bail!("неподдержанный video.encoder `{other}`"),
    }
}

pub(crate) fn select_encoder_element_name(
    video: &VideoConfig,
    availability: &impl ElementAvailability,
) -> Result<&'static str> {
    Ok(select_encoder(video, availability)?.element_name())
}

fn build_d3d11_encoder_launch_string(
    video: &VideoConfig,
    capture: &CaptureConfig,
    encoder: &str,
    encoder_properties: String,
) -> String {
    format!(
        "( {source} \
         ! queue max-size-buffers=2 leaky=downstream \
         ! d3d11convert \
         ! video/x-raw(memory:D3D11Memory),format=NV12,width={width},height={height},framerate={fps}/1 \
         ! {encoder} {encoder_properties} \
         ! h264parse config-interval=1 \
         ! rtph264pay name=pay0 pt=96 config-interval=1 )",
        source = capture_source(capture),
        width = video.width,
        height = video.height,
        fps = video.fps,
    )
}

fn build_mfh264_launch_string(video: &VideoConfig, capture: &CaptureConfig) -> String {
    build_d3d11_encoder_launch_string(
        video,
        capture,
        Encoder::MediaFoundationH264.element_name(),
        format!(
            "bitrate={} rc-mode=cbr low-latency=true gop-size={}",
            video.bitrate_kbps, video.fps
        ),
    )
}

fn build_nvenc_launch_string(
    video: &VideoConfig,
    capture: &CaptureConfig,
    encoder: Encoder,
) -> String {
    build_d3d11_encoder_launch_string(
        video,
        capture,
        encoder.element_name(),
        format!(
            "bitrate={} rc-mode=cbr tune=ultra-low-latency zerolatency=true gop-size={} bframes=0",
            video.bitrate_kbps, video.fps
        ),
    )
}

fn build_qsv_launch_string(video: &VideoConfig, capture: &CaptureConfig) -> String {
    build_d3d11_encoder_launch_string(
        video,
        capture,
        Encoder::IntelQsvH264.element_name(),
        format!(
            "bitrate={} rate-control=cbr gop-size={} b-frames=0",
            video.bitrate_kbps, video.fps
        ),
    )
}

fn build_amf_launch_string(video: &VideoConfig, capture: &CaptureConfig) -> String {
    build_d3d11_encoder_launch_string(
        video,
        capture,
        Encoder::AmdAmfH264.element_name(),
        format!(
            "bitrate={} rate-control=cbr usage=ultra-low-latency gop-size={} b-frames=0",
            video.bitrate_kbps, video.fps
        ),
    )
}

fn build_x264_launch_string(video: &VideoConfig, capture: &CaptureConfig) -> String {
    format!(
        "( {source} \
         ! queue max-size-buffers=2 leaky=downstream \
         ! d3d11convert \
         ! video/x-raw(memory:D3D11Memory),width={width},height={height},framerate={fps}/1 \
         ! d3d11download \
         ! videoconvert \
         ! video/x-raw,format=I420 \
         ! x264enc tune=zerolatency speed-preset=veryfast bitrate={bitrate} key-int-max={fps} bframes=0 \
         ! h264parse config-interval=1 \
         ! rtph264pay name=pay0 pt=96 config-interval=1 )",
        source = capture_source(capture),
        width = video.width,
        height = video.height,
        fps = video.fps,
        bitrate = video.bitrate_kbps,
    )
}

fn capture_source(capture: &CaptureConfig) -> String {
    format!(
        "d3d11screencapturesrc monitor-index={} show-cursor={} capture-api={}",
        capture.monitor_index, capture.capture_cursor, capture.capture_api
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[derive(Debug)]
    struct FakeAvailability {
        elements: HashSet<&'static str>,
    }

    impl FakeAvailability {
        fn new(elements: impl IntoIterator<Item = &'static str>) -> Self {
            Self {
                elements: elements.into_iter().collect(),
            }
        }
    }

    impl ElementAvailability for FakeAvailability {
        fn is_available(&self, element: &str) -> bool {
            self.elements.contains(element)
        }
    }

    #[test]
    fn launch_string_should_use_mfh264_encoder_by_default() {
        // Given
        let video = VideoConfig::default();
        let capture = CaptureConfig::default();
        let availability = FakeAvailability::new(["mfh264enc", "x264enc"]);

        // When
        let launch = build_launch_string(&video, &capture, &availability).unwrap();

        // Then
        assert!(launch.contains("d3d11screencapturesrc"));
        assert!(launch.contains("monitor-index=-1"));
        assert!(launch.contains("show-cursor=true"));
        assert!(launch.contains("capture-api=dxgi"));
        assert!(launch.contains(
            "video/x-raw(memory:D3D11Memory),format=NV12,width=1280,height=720,framerate=15/1"
        ));
        assert!(launch.contains("mfh264enc bitrate=2500"));
        assert!(launch.contains("rtph264pay name=pay0"));
    }

    #[test]
    fn launch_string_should_use_x264_for_software_only() {
        // Given
        let video = VideoConfig {
            encoder: "software_only".to_owned(),
            ..VideoConfig::default()
        };
        let capture = CaptureConfig::default();
        let availability = FakeAvailability::new(["x264enc"]);

        // When
        let launch = build_launch_string(&video, &capture, &availability).unwrap();

        // Then
        assert!(launch.contains("d3d11download"));
        assert!(launch.contains("videoconvert"));
        assert!(launch.contains("x264enc tune=zerolatency"));
        assert!(launch.contains("rtph264pay name=pay0"));
    }

    #[test]
    fn select_encoder_should_follow_project_fallback_order() {
        // Given
        let video = VideoConfig::default();
        let availability = FakeAvailability::new(["x264enc", "amfh264enc", "nvd3d11h264enc"]);

        // When
        let encoder = select_encoder(&video, &availability).unwrap();

        // Then
        assert_eq!(encoder, Encoder::NvidiaD3d11H264);
    }

    #[test]
    fn launch_string_should_use_nvidia_encoder_when_selected() {
        // Given
        let video = VideoConfig::default();
        let capture = CaptureConfig::default();
        let availability = FakeAvailability::new(["nvd3d11h264enc"]);

        // When
        let launch = build_launch_string(&video, &capture, &availability).unwrap();

        // Then
        assert!(launch.contains("nvd3d11h264enc"));
        assert!(launch.contains("tune=ultra-low-latency"));
        assert!(launch.contains("zerolatency=true"));
    }

    #[test]
    fn launch_string_should_use_qsv_encoder_when_selected() {
        // Given
        let video = VideoConfig::default();
        let capture = CaptureConfig::default();
        let availability = FakeAvailability::new(["qsvh264enc"]);

        // When
        let launch = build_launch_string(&video, &capture, &availability).unwrap();

        // Then
        assert!(launch.contains("qsvh264enc"));
        assert!(launch.contains("rate-control=cbr"));
        assert!(launch.contains("b-frames=0"));
    }

    #[test]
    fn launch_string_should_use_amf_encoder_when_selected() {
        // Given
        let video = VideoConfig::default();
        let capture = CaptureConfig::default();
        let availability = FakeAvailability::new(["amfh264enc"]);

        // When
        let launch = build_launch_string(&video, &capture, &availability).unwrap();

        // Then
        assert!(launch.contains("amfh264enc"));
        assert!(launch.contains("usage=ultra-low-latency"));
        assert!(launch.contains("b-frames=0"));
    }

    #[test]
    fn select_encoder_should_error_when_no_encoder_is_available() {
        // Given
        let video = VideoConfig::default();
        let availability = FakeAvailability::new([]);

        // When
        let result = select_encoder(&video, &availability);

        // Then
        assert!(result.is_err());
    }
}
