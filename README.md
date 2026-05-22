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
