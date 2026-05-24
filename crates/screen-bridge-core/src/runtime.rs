//! Настройка app-local runtime перед запуском GStreamer.
//!
//! Installer кладет `screen-bridge-host.exe`, `screen-bridge-viewer.exe` и
//! GStreamer DLL в один каталог `bin`, а plugins оставляет в соседнем
//! `lib\gstreamer-1.0`. Этот module находит такой layout и задает переменные
//! окружения до `gst::init()`.

use std::env;
use std::path::{Path, PathBuf};

const GSTREAMER_CORE_DLL: &str = "gstreamer-1.0-0.dll";
const GSTREAMER_PLUGIN_DIRECTORY: &str = "gstreamer-1.0";

/// Найденный app-local layout GStreamer runtime.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BundledGStreamerEnvironment {
    root: PathBuf,
    bin: PathBuf,
    plugins: PathBuf,
    pkg_config: PathBuf,
    scanner: PathBuf,
    gio_modules: PathBuf,
}

impl BundledGStreamerEnvironment {
    /// Корень установленного app-local GStreamer layout.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Каталог с GStreamer DLL и установленными ScreenBridge executable.
    pub fn bin(&self) -> &Path {
        &self.bin
    }

    /// Каталог с GStreamer plugins.
    pub fn plugins(&self) -> &Path {
        &self.plugins
    }

    /// Каталог с `pkg-config` metadata bundled GStreamer.
    pub fn pkg_config(&self) -> &Path {
        &self.pkg_config
    }

    /// Путь к `gst-plugin-scanner.exe` внутри bundled runtime.
    pub fn scanner(&self) -> &Path {
        &self.scanner
    }

    /// Пустой каталог для отключения optional GIO modules.
    pub fn gio_modules(&self) -> &Path {
        &self.gio_modules
    }
}

/// Если приложение запущено из installer layout, настраивает bundled GStreamer.
///
/// В обычной dev-сессии функция ничего не меняет: `scripts/env-gstreamer.ps1`
/// остается источником `PATH`, `PKG_CONFIG_PATH` и локального GStreamer root.
pub fn configure_bundled_gstreamer_environment() -> Option<BundledGStreamerEnvironment> {
    let environment = detect_bundled_gstreamer_environment()?;
    apply_bundled_gstreamer_environment(&environment);
    Some(environment)
}

/// Ищет bundled GStreamer layout рядом с текущим executable.
pub fn detect_bundled_gstreamer_environment() -> Option<BundledGStreamerEnvironment> {
    let executable = env::current_exe().ok()?;
    detect_bundled_gstreamer_environment_from_exe(executable)
}

/// Ищет bundled GStreamer layout для указанного executable path.
///
/// Ожидаемый layout:
///
/// ```text
/// ScreenBridge\
///   bin\
///     screen-bridge-host.exe
///     screen-bridge-viewer.exe
///     gstreamer-1.0-0.dll
///   lib\gstreamer-1.0\
///   lib\pkgconfig\
///   libexec\gstreamer-1.0\gst-plugin-scanner.exe
///   empty-gio-modules\
/// ```
pub fn detect_bundled_gstreamer_environment_from_exe(
    executable: impl AsRef<Path>,
) -> Option<BundledGStreamerEnvironment> {
    let executable_directory = executable.as_ref().parent()?;
    if !is_bin_directory(executable_directory) {
        return None;
    }

    let root = executable_directory.parent()?.to_path_buf();
    let bin = root.join("bin");
    let plugins = root.join("lib").join(GSTREAMER_PLUGIN_DIRECTORY);
    let pkg_config = root.join("lib").join("pkgconfig");
    let scanner = root
        .join("libexec")
        .join(GSTREAMER_PLUGIN_DIRECTORY)
        .join("gst-plugin-scanner.exe");
    let gio_modules = root.join("empty-gio-modules");

    if !bin.join(GSTREAMER_CORE_DLL).is_file() || !plugins.is_dir() {
        return None;
    }

    Some(BundledGStreamerEnvironment {
        root,
        bin,
        plugins,
        pkg_config,
        scanner,
        gio_modules,
    })
}

fn apply_bundled_gstreamer_environment(environment: &BundledGStreamerEnvironment) {
    env::set_var("GSTREAMER_1_0_ROOT_MSVC_X86_64", environment.root());
    env::set_var("GSTREAMER_ROOT_X86_64", environment.root());
    env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", environment.plugins());
    env::set_var("GST_PLUGIN_PATH_1_0", environment.plugins());
    env::set_var("GIO_MODULE_DIR", environment.gio_modules());

    if environment.pkg_config().is_dir() {
        env::set_var("PKG_CONFIG_PATH", environment.pkg_config());
    }

    if environment.scanner().is_file() {
        env::set_var("GST_PLUGIN_SCANNER", environment.scanner());
        env::set_var("GST_PLUGIN_SCANNER_1_0", environment.scanner());
    }
}

fn is_bin_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.eq_ignore_ascii_case("bin"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn detect_bundled_environment_should_accept_installer_layout() {
        // Given
        let root = test_root("valid-layout");
        let bin = root.join("bin");
        let plugins = root.join("lib").join(GSTREAMER_PLUGIN_DIRECTORY);
        let pkg_config = root.join("lib").join("pkgconfig");
        let scanner_directory = root.join("libexec").join(GSTREAMER_PLUGIN_DIRECTORY);
        let executable = bin.join("screen-bridge-host.exe");
        create_file(&bin.join(GSTREAMER_CORE_DLL));
        create_file(&executable);
        fs::create_dir_all(&plugins).unwrap();
        fs::create_dir_all(&pkg_config).unwrap();
        fs::create_dir_all(&scanner_directory).unwrap();
        create_file(&scanner_directory.join("gst-plugin-scanner.exe"));

        // When
        let result = detect_bundled_gstreamer_environment_from_exe(&executable).unwrap();

        // Then
        assert_eq!(result.root(), root.as_path());
        assert_eq!(result.bin(), bin.as_path());
        assert_eq!(result.plugins(), plugins.as_path());
        assert_eq!(result.pkg_config(), pkg_config.as_path());
        assert_eq!(
            result.scanner(),
            scanner_directory.join("gst-plugin-scanner.exe").as_path()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn detect_bundled_environment_should_ignore_dev_target_layout() {
        // Given
        let root = test_root("dev-layout");
        let debug = root.join("target").join("debug");
        let executable = debug.join("screen-bridge-host.exe");
        create_file(&debug.join(GSTREAMER_CORE_DLL));
        create_file(&executable);

        // When
        let result = detect_bundled_gstreamer_environment_from_exe(&executable);

        // Then
        assert_eq!(result, None);

        let _ = fs::remove_dir_all(root);
    }

    fn test_root(name: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("runtime-tests")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn create_file(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }
}
