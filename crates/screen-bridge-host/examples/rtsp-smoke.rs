//! Временный RTSP smoke server для проверки GStreamer pipeline.
//!
//! Example публикует screen capture stream без auth/security, чтобы отдельно
//! проверить `gstreamer-rtsp-server`, H.264 encode и RTSP/TCP client до
//! реализации production host.

use std::env;

use anyhow::{bail, Context, Result};
use gstreamer_rtsp_server::gst_rtsp;
use gstreamer_rtsp_server::prelude::*;
use gstreamer_rtsp_server::{glib, gst, RTSPMediaFactory, RTSPServer};

#[derive(Debug)]
struct Options {
    address: String,
    port: u16,
    path: String,
    monitor_index: i32,
    capture_api: String,
    show_cursor: bool,
    width: u32,
    height: u32,
    fps: u32,
    bitrate_kbps: u32,
    duration_seconds: Option<u32>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_owned(),
            port: 8554,
            path: "/screen".to_owned(),
            monitor_index: -1,
            capture_api: "dxgi".to_owned(),
            show_cursor: true,
            width: 1280,
            height: 720,
            fps: 15,
            bitrate_kbps: 2500,
            duration_seconds: None,
        }
    }
}

fn main() -> Result<()> {
    let options = parse_options(env::args().skip(1))?;
    gst::init().context("failed to initialize GStreamer")?;

    let launch = build_launch_string(&options);
    let main_loop = glib::MainLoop::new(None, false);
    let server = RTSPServer::new();
    server.set_address(&options.address);
    server.set_service(&options.port.to_string());

    let mounts = server
        .mount_points()
        .context("failed to get RTSP mount points")?;

    let factory = RTSPMediaFactory::new();
    factory.set_launch(&launch);
    factory.set_shared(true);
    factory.set_stop_on_disconnect(true);
    factory.set_protocols(gst_rtsp::RTSPLowerTrans::TCP);
    mounts.add_factory(&options.path, factory);

    let source_id = server
        .attach(None)
        .context("failed to attach RTSP server to main context")?;

    let stop_loop = main_loop.clone();
    ctrlc::set_handler(move || {
        stop_loop.quit();
    })
    .context("failed to install Ctrl+C handler")?;

    if let Some(seconds) = options.duration_seconds {
        let timeout_loop = main_loop.clone();
        glib::timeout_add_seconds_once(seconds, move || {
            timeout_loop.quit();
        });
    }

    println!("RTSP smoke server is running.");
    println!("Bind: {}:{}", options.address, options.port);
    println!("Path: {}", options.path);
    println!(
        "URL for VLC: rtsp://<host-ip>:{}{}",
        options.port, options.path
    );
    println!("Transport: TCP only");
    println!("Launch: {}", launch);
    println!("Press Ctrl+C to stop.");

    main_loop.run();
    source_id.remove();
    println!("RTSP smoke server stopped.");

    Ok(())
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Options> {
    let mut options = Options::default();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--address" => options.address = next_value(&mut args, "--address")?,
            "--port" => options.port = parse_next(&mut args, "--port")?,
            "--path" => options.path = next_value(&mut args, "--path")?,
            "--monitor-index" => options.monitor_index = parse_next(&mut args, "--monitor-index")?,
            "--capture-api" => options.capture_api = next_value(&mut args, "--capture-api")?,
            "--no-cursor" => options.show_cursor = false,
            "--width" => options.width = parse_next(&mut args, "--width")?,
            "--height" => options.height = parse_next(&mut args, "--height")?,
            "--fps" => options.fps = parse_next(&mut args, "--fps")?,
            "--bitrate-kbps" => options.bitrate_kbps = parse_next(&mut args, "--bitrate-kbps")?,
            "--duration-seconds" => {
                let seconds: u32 = parse_next(&mut args, "--duration-seconds")?;
                options.duration_seconds = if seconds == 0 { None } else { Some(seconds) };
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            unknown => bail!("unknown argument: {unknown}"),
        }
    }

    validate_options(&options)?;
    Ok(options)
}

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String> {
    args.next()
        .with_context(|| format!("missing value for {name}"))
}

fn parse_next<T>(args: &mut impl Iterator<Item = String>, name: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let value = next_value(args, name)?;
    value
        .parse::<T>()
        .with_context(|| format!("invalid value for {name}: {value}"))
}

fn validate_options(options: &Options) -> Result<()> {
    if !options.path.starts_with('/') {
        bail!("--path must start with /");
    }

    if options.capture_api != "dxgi" && options.capture_api != "wgc" {
        bail!("--capture-api must be dxgi or wgc");
    }

    if options.width < 64 || options.height < 64 {
        bail!("--width and --height must be at least 64");
    }

    if options.fps == 0 {
        bail!("--fps must be greater than 0");
    }

    if options.bitrate_kbps == 0 {
        bail!("--bitrate-kbps must be greater than 0");
    }

    Ok(())
}

fn build_launch_string(options: &Options) -> String {
    format!(
        "( d3d11screencapturesrc monitor-index={} show-cursor={} capture-api={} \
         ! queue max-size-buffers=2 leaky=downstream \
         ! d3d11convert \
         ! video/x-raw(memory:D3D11Memory),format=NV12,width={},height={},framerate={}/1 \
         ! mfh264enc bitrate={} rc-mode=cbr low-latency=true gop-size={} \
         ! h264parse config-interval=1 \
         ! rtph264pay name=pay0 pt=96 config-interval=1 )",
        options.monitor_index,
        options.show_cursor,
        options.capture_api,
        options.width,
        options.height,
        options.fps,
        options.bitrate_kbps,
        options.fps,
    )
}

fn print_help() {
    println!("RTSP smoke server for ScreenBridge");
    println!();
    println!("Options:");
    println!("  --address <ip>              Bind address, default 0.0.0.0");
    println!("  --port <port>               RTSP port, default 8554");
    println!("  --path <path>               RTSP path, default /screen");
    println!("  --monitor-index <index>     Monitor index, default -1");
    println!("  --capture-api <dxgi|wgc>    Capture backend, default dxgi");
    println!("  --no-cursor                 Hide cursor in captured stream");
    println!("  --width <pixels>            Width, default 1280");
    println!("  --height <pixels>           Height, default 720");
    println!("  --fps <fps>                 Frame rate, default 15");
    println!("  --bitrate-kbps <kbps>       H.264 bitrate, default 2500");
    println!("  --duration-seconds <sec>    Stop automatically, 0 means Ctrl+C");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_string_should_contain_rtsp_payload_name() {
        let launch = build_launch_string(&Options::default());

        assert!(launch.contains("rtph264pay name=pay0"));
    }

    #[test]
    fn launch_string_should_use_requested_capture_settings() {
        let options = Options {
            capture_api: "wgc".to_owned(),
            monitor_index: 1,
            show_cursor: false,
            width: 1920,
            height: 1080,
            fps: 30,
            bitrate_kbps: 4000,
            ..Options::default()
        };

        let launch = build_launch_string(&options);

        assert!(launch.contains("monitor-index=1"));
        assert!(launch.contains("show-cursor=false"));
        assert!(launch.contains("capture-api=wgc"));
        assert!(launch.contains("width=1920,height=1080,framerate=30/1"));
        assert!(launch.contains("bitrate=4000"));
        assert!(launch.contains("gop-size=30"));
    }

    #[test]
    fn parse_options_should_reject_path_without_leading_slash() {
        let result = parse_options(["--path".to_owned(), "screen".to_owned()]);

        assert!(result.is_err());
    }
}
