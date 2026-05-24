//! Диагностика окружения host без запуска RTSP server.

use std::env;
use std::fmt;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Command;

use screen_bridge_core::config::{load_host, ConfigWarning, HostConfig};
use screen_bridge_core::net;

use crate::pipeline::{self, GstElementAvailability};
use crate::{build_masked_rtsp_url, resolve_bind_ip};

const REQUIRED_ELEMENTS: &[&str] = &[
    "d3d11screencapturesrc",
    "d3d11convert",
    "d3d11download",
    "d3d11videosink",
    "videoconvert",
    "mfh264enc",
    "x264enc",
    "rtph264pay",
    "rtph264depay",
    "h264parse",
    "decodebin",
    "rtspsrc",
];

const OPTIONAL_ELEMENTS: &[&str] = &[
    "nvd3d11h264enc",
    "nvautogpuh264enc",
    "qsvh264enc",
    "amfh264enc",
    "d3d11h264dec",
    "avdec_h264",
];

/// Результат host diagnostics.
pub struct DiagnosticReport {
    lines: Vec<String>,
    passed: usize,
    warnings: usize,
    failed: usize,
}

impl DiagnosticReport {
    fn new() -> Self {
        Self {
            lines: vec![
                "ScreenBridge host diagnostics".to_owned(),
                format!("Working directory: {}", working_directory()),
                String::new(),
            ],
            passed: 0,
            warnings: 0,
            failed: 0,
        }
    }

    /// Возвращает `true`, если diagnostics нашли blocker.
    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }

    fn pass(&mut self, name: &str, message: impl Into<String>) {
        self.passed += 1;
        self.lines
            .push(format!("[PASS] {name} - {}", message.into()));
    }

    fn warn(&mut self, name: &str, message: impl Into<String>) {
        self.warnings += 1;
        self.lines
            .push(format!("[WARN] {name} - {}", message.into()));
    }

    fn fail(&mut self, name: &str, message: impl Into<String>) {
        self.failed += 1;
        self.lines
            .push(format!("[FAIL] {name} - {}", message.into()));
    }

    fn info(&mut self, name: &str, message: impl Into<String>) {
        self.lines
            .push(format!("[INFO] {name} - {}", message.into()));
    }
}

impl fmt::Display for DiagnosticReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.lines {
            writeln!(formatter, "{line}")?;
        }

        writeln!(formatter)?;
        writeln!(
            formatter,
            "Summary: {} passed, {} warning(s), {} failed",
            self.passed, self.warnings, self.failed
        )
    }
}

/// Запускает host diagnostics. Config path можно не передавать.
pub fn diagnose(config_path: Option<&Path>) -> DiagnosticReport {
    let mut report = DiagnosticReport::new();
    let config = check_config(&mut report, config_path);

    check_rust(&mut report);
    check_application(
        &mut report,
        "GStreamer gst-launch",
        &["gst-launch-1.0.exe", "gst-launch-1.0"],
    );
    check_application(
        &mut report,
        "GStreamer gst-inspect",
        &["gst-inspect-1.0.exe", "gst-inspect-1.0"],
    );
    check_pkg_config(&mut report);
    let gstreamer_ready = check_gstreamer(&mut report);
    if gstreamer_ready {
        check_elements(&mut report);
        check_selected_encoder(&mut report, config.as_ref());
    }
    check_network(&mut report);
    check_config_dependent_network(&mut report, config.as_ref());

    report
}

fn check_config(report: &mut DiagnosticReport, config_path: Option<&Path>) -> Option<HostConfig> {
    let Some(path) = config_path else {
        report.warn(
            "Host config",
            "not provided; selected bind IP and port checks use default host settings",
        );
        return None;
    };

    match load_host(path) {
        Ok(config) => {
            report.pass("Host config", format!("loaded {}", path.display()));
            if config.has_warning(ConfigWarning::AllowSubnetAny) {
                report.warn(
                    "allow_subnet",
                    "security.allow_subnet = \"any\" accepts every peer; use a LAN CIDR before release",
                );
            } else {
                report.pass(
                    "allow_subnet",
                    format!("security.allow_subnet = {}", config.security.allow_subnet),
                );
            }

            Some(config)
        }
        Err(error) => {
            report.fail("Host config", error.to_string());
            None
        }
    }
}

fn check_rust(report: &mut DiagnosticReport) {
    match command_output("rustc", &["-Vv"]) {
        Ok(output) => {
            let release = output
                .lines()
                .find(|line| line.starts_with("release:"))
                .unwrap_or("release: unknown");
            let host = output
                .lines()
                .find(|line| line.starts_with("host:"))
                .unwrap_or("host: unknown");

            if host.contains("x86_64-pc-windows-msvc") {
                report.pass("Rust rustc", format!("{release}, {host}"));
            } else {
                report.fail(
                    "Rust target",
                    format!("expected x86_64-pc-windows-msvc, got {host}"),
                );
            }
        }
        Err(error) => report.fail("Rust rustc", error),
    }

    match command_output("cargo", &["-V"]) {
        Ok(output) => report.pass("Rust cargo", output),
        Err(error) => report.fail("Rust cargo", error),
    }
}

fn check_application(report: &mut DiagnosticReport, name: &str, names: &[&str]) {
    match find_application(names) {
        Some(path) => report.pass(name, path.display().to_string()),
        None => report.fail(name, format!("{} was not found in PATH", names[0])),
    }
}

fn check_pkg_config(report: &mut DiagnosticReport) {
    check_application(report, "pkg-config", &["pkg-config.exe", "pkg-config"]);

    let core = command_output("pkg-config", &["--modversion", "gstreamer-1.0"]);
    let rtsp = command_output("pkg-config", &["--modversion", "gstreamer-rtsp-server-1.0"]);

    match &core {
        Ok(version) => report.pass("pkg-config gstreamer-1.0", version),
        Err(error) => report.fail("pkg-config gstreamer-1.0", error),
    }

    match &rtsp {
        Ok(version) => report.pass("pkg-config gstreamer-rtsp-server-1.0", version),
        Err(error) => report.fail("pkg-config gstreamer-rtsp-server-1.0", error),
    }

    if let (Ok(core), Ok(rtsp)) = (core, rtsp) {
        if core == rtsp {
            report.pass(
                "GStreamer package versions",
                format!("Core and RTSP server versions match: {core}"),
            );
        } else {
            report.warn(
                "GStreamer package versions",
                format!("Core is {core}, RTSP server is {rtsp}. Use matching installers."),
            );
        }
    }
}

fn check_gstreamer(report: &mut DiagnosticReport) -> bool {
    match gstreamer::init() {
        Ok(()) => {
            report.pass("GStreamer init", gstreamer::version_string());
            check_gstreamer_root(report);
            true
        }
        Err(error) => {
            report.fail("GStreamer init", error.to_string());
            false
        }
    }
}

fn check_gstreamer_root(report: &mut DiagnosticReport) {
    let root =
        env::var("GSTREAMER_1_0_ROOT_MSVC_X86_64").or_else(|_| env::var("GSTREAMER_ROOT_X86_64"));

    match root {
        Ok(root) if !root.trim().is_empty() => report.pass("GStreamer root", root),
        _ => report.warn(
            "GStreamer root",
            "GSTREAMER_1_0_ROOT_MSVC_X86_64 is not set; PATH must still include GStreamer bin",
        ),
    }

    match env::var("GST_PLUGIN_PATH") {
        Ok(path) if !path.trim().is_empty() => report.info("GStreamer plugin path", path),
        _ => report.info("GStreamer plugin path", "GST_PLUGIN_PATH is not set"),
    }
}

fn check_elements(report: &mut DiagnosticReport) {
    for element in REQUIRED_ELEMENTS {
        if gstreamer::ElementFactory::find(element).is_some() {
            report.pass(&format!("GStreamer element {element}"), "available");
        } else {
            report.fail(&format!("GStreamer element {element}"), "missing");
        }
    }

    let available_optional = OPTIONAL_ELEMENTS
        .iter()
        .copied()
        .filter(|element| gstreamer::ElementFactory::find(element).is_some())
        .collect::<Vec<_>>();

    if available_optional.is_empty() {
        report.warn(
            "Optional GStreamer elements",
            "No vendor H.264 encoder or explicit H.264 decoder was found",
        );
    } else {
        report.pass("Optional GStreamer elements", available_optional.join(", "));
    }
}

fn check_selected_encoder(report: &mut DiagnosticReport, config: Option<&HostConfig>) {
    let config = config.cloned().unwrap_or_default();
    let availability = GstElementAvailability;

    match pipeline::select_encoder_element_name(&config.video, &availability) {
        Ok(encoder) => report.pass("Selected encoder", encoder),
        Err(error) => report.fail("Selected encoder", error.to_string()),
    }
}

fn check_network(report: &mut DiagnosticReport) {
    match net::local_ipv4() {
        Ok(addresses) if !addresses.is_empty() => {
            let addresses = addresses
                .iter()
                .map(Ipv4Addr::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            report.pass("Local IPv4 interfaces", addresses);
        }
        Ok(_) => report.warn(
            "Local IPv4 interfaces",
            "No non-loopback private IPv4 address was found",
        ),
        Err(error) => report.fail("Local IPv4 interfaces", error.to_string()),
    }
}

fn check_config_dependent_network(report: &mut DiagnosticReport, config: Option<&HostConfig>) {
    let runtime_config = config.cloned().unwrap_or_default();

    match resolve_bind_ip(&runtime_config) {
        Ok(bind_ip) => {
            report.pass("Selected bind IP", bind_ip.to_string());
            check_port(report, bind_ip, runtime_config.server.port);
            if let Some(config) = config {
                report.info("Masked RTSP URL", build_masked_rtsp_url(config, bind_ip));
            } else {
                report.info(
                    "Masked RTSP URL",
                    "unavailable without a validated host config",
                );
            }
            report.info(
                "Firewall hint",
                format!(
                    "New-NetFirewallRule -DisplayName \"ScreenBridge Host RTSP\" -Direction Inbound -Action Allow -Protocol TCP -LocalPort {}",
                    runtime_config.server.port
                ),
            );
        }
        Err(error) => report.fail("Selected bind IP", error.to_string()),
    }
}

fn check_port(report: &mut DiagnosticReport, bind_ip: Ipv4Addr, port: u16) {
    let address = SocketAddrV4::new(bind_ip, port);

    match TcpListener::bind(address) {
        Ok(listener) => {
            drop(listener);
            report.pass("TCP port", format!("{address} is available"));
        }
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            report.fail("TCP port", format!("{address} is already in use"));
        }
        Err(error) => report.fail("TCP port", format!("cannot bind {address}: {error}")),
    }
}

fn command_output(command: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|error| format!("{command} failed to start: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();

    if output.status.success() {
        return Ok(stdout);
    }

    let message = if stderr.is_empty() { stdout } else { stderr };
    Err(format!(
        "{command} exited with {}: {message}",
        output.status
    ))
}

fn find_application(names: &[&str]) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let path_ext = env::var_os("PATHEXT")
        .map(|value| {
            env::split_paths(&value)
                .map(|path| path.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![".EXE".to_owned()]);

    for directory in env::split_paths(&path_var) {
        for name in names {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }

            if Path::new(name).extension().is_none() {
                for extension in &path_ext {
                    let candidate = directory.join(format!("{name}{extension}"));
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    None
}

fn working_directory() -> String {
    env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|error| format!("unknown: {error}"))
}
