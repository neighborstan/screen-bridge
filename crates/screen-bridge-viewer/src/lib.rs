//! Viewer runtime для подключения к host RTSP stream.
//!
//! Crate строит GStreamer playback pipeline, запускает его и завершает процесс
//! понятным результатом при EOS, Ctrl+C или ошибке воспроизведения.
#![warn(missing_docs)]

mod pipeline;
mod run;

pub use pipeline::{build_launch, VideoSink, ViewerLaunch};
pub use run::run;
