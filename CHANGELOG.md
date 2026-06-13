# Changelog

Все заметные изменения проекта фиксируются в этом файле.

Формат основан на Keep a Changelog, версии следуют SemVer.

## [0.1.0] - 2026-06-13

### Added

- Первый Windows installer `ScreenBridge-0.1.0-windows-x64-setup.exe`.
- Host-приложение `screen-bridge-host.exe` для публикации экрана по RTSP/TCP.
- Viewer-приложение `screen-bridge-viewer.exe` для просмотра RTSP/TCP stream.
- H.264 video pipeline на official GStreamer MSVC runtime.
- Basic auth, ограничение одного viewer и IPv4 subnet allowlist.
- App-local GStreamer runtime в installer, чтобы установленная версия не
  требовала системный GStreamer `PATH`.
- Конфиги `host.toml` и `viewer.toml` в `%APPDATA%\ScreenBridge`.
- Start Menu shortcuts для Host, Viewer, Config и разрешения Windows Firewall.
- Host diagnostics и viewer TCP preflight для более понятной диагностики
  сетевых ошибок.

### Fixed

- Viewer timeout до RTSP authentication теперь объясняется как недоступный TCP
  port с подсказкой про Windows Firewall и `Test-NetConnection`.
- Повторная установка очищает app-local runtime files перед копированием новой
  версии, не трогая пользовательские конфиги.
