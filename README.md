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
