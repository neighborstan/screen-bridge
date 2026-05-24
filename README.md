# ScreenBridge

ScreenBridge - Windows-приложение для просмотра экрана другого компьютера в
домашней LAN.

Стек: Rust + official GStreamer MSVC + H.264 + RTSP/TCP + Basic auth.

## Требования

- Windows 10/11 x64.
- Rust stable `x86_64-pc-windows-msvc`.
- Official GStreamer Windows MSVC runtime + devel одной версии.

## Проверка окружения

```powershell
.\scripts\check-gstreamer.ps1
```

Скрипт автоматически использует локальную установку GStreamer из
`.local\gstreamer\msvc_x86_64`, если она есть.

Exit code `0` - окружение готово. Exit code `1` - есть критическая проблема,
которую нужно исправить перед GStreamer smoke.

## Сборка installer

```powershell
.\scripts\build-installer.ps1
```

Скрипт проверяет prerequisites, при отсутствии Inno Setup 6 предложит
установить его через `winget`, соберет release binaries и упакует app-local
GStreamer runtime.

Готовый installer появится в `dist\`:

```text
dist\ScreenBridge-0.1.0-windows-x64-setup.exe
```

Если нужно только проверить prereqs без автоматической установки tools:

```powershell
.\scripts\build-installer.ps1 -NoInstallMissingTools
```

## Host RTSP server

Создайте локальный config из примера:

```powershell
Copy-Item config\host.example.toml config\host.local.toml
```

Перед запуском измените:

- `security.access_token` - secret token не короче 16 символов, не placeholder.
- `security.allow_subnet` - разрешенная IPv4 subnet, например
  `"192.168.1.0/24"`. Значение `"any"` отключает subnet filtering и дает
  warning.
- `server.bind_ip` - можно оставить закомментированным для автоматического
  выбора LAN IPv4 или задать явно.

Запуск host:

```powershell
.\scripts\env-gstreamer.ps1
cargo run -p screen-bridge-host -- --config config\host.local.toml
```

Host печатает bind address, path, masked RTSP URL, Basic auth user и masked token.
RTSP Basic auth обязателен, `server.max_clients = 1` enforced, clients вне
`security.allow_subnet` отклоняются.

Проверка host diagnostics:

```powershell
cargo run -p screen-bridge-host -- --diagnose --config config\host.local.toml
```

Полный VLC URL с token выводится только явной командой:

```powershell
cargo run -p screen-bridge-host -- --print-vlc-url --config config\host.local.toml
```

В VLC на другом компьютере в LAN откройте этот URL и используйте RTSP/TCP
transport.

## Viewer

Создайте локальный config из примера:

```powershell
Copy-Item config\viewer.example.toml config\viewer.local.toml
```

Перед запуском измените:

- `connection.host` - LAN IP компьютера, где запущен host.
- `connection.access_token` - тот же token, что в host config.

Запуск viewer:

```powershell
.\scripts\env-gstreamer.ps1
cargo run -p screen-bridge-viewer -- --config config\viewer.local.toml
```

Viewer подключается по RTSP/TCP с Basic auth и открывает окно с видео host
screen.

## GStreamer smoke

Локальная проверка захвата экрана:

```powershell
.\scripts\smoke-local-capture.ps1
```

Откроется окно preview. Для остановки нажать `Ctrl+C`.
Если preview-окно попадает в область захвата, будет виден зеркальный повтор
одного и того же окна. Это нормальный признак того, что экран действительно
захватывается.

Проверка кодирования H.264 без окна:

```powershell
.\scripts\smoke-local-capture.ps1 -Encode
```

По умолчанию encode smoke работает 10 секунд. Длительность можно задать явно:

```powershell
.\scripts\smoke-local-capture.ps1 -Encode -DurationSeconds 15
```

Если DXGI-захват не работает в текущей сессии, можно попробовать WGC:

```powershell
.\scripts\smoke-local-capture.ps1 -CaptureApi wgc
```

RTSP smoke server без auth/security:

```powershell
.\scripts\smoke-rtsp-server.ps1
```

На втором компьютере в LAN открыть в VLC:

```text
rtsp://<host-ip>:8554/screen
```

В VLC нужно использовать RTSP/TCP transport. Скрипт останавливается через
`Ctrl+C`.

## Лицензия

Проект распространяется по лицензии MIT. См. `LICENSE`.
