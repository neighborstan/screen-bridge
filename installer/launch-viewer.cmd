@echo off
setlocal

echo ScreenBridge Viewer
echo Config: %APPDATA%\ScreenBridge\viewer.toml
echo.

"%~dp0screen-bridge-viewer.exe" --config "%APPDATA%\ScreenBridge\viewer.toml"
set "SCREENBRIDGE_EXIT_CODE=%ERRORLEVEL%"

echo.
echo ScreenBridge Viewer exited with code %SCREENBRIDGE_EXIT_CODE%.
echo Press any key to close this window.
pause >nul
exit /b %SCREENBRIDGE_EXIT_CODE%
