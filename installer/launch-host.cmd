@echo off
setlocal

echo ScreenBridge Host
echo Config: %APPDATA%\ScreenBridge\host.toml
echo.

"%~dp0screen-bridge-host.exe" --config "%APPDATA%\ScreenBridge\host.toml"
set "SCREENBRIDGE_EXIT_CODE=%ERRORLEVEL%"

echo.
echo ScreenBridge Host exited with code %SCREENBRIDGE_EXIT_CODE%.
echo Press any key to close this window.
pause >nul
exit /b %SCREENBRIDGE_EXIT_CODE%
