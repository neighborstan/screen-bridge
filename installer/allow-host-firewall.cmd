@echo off
setlocal

set "SCREENBRIDGE_FIREWALL_SCRIPT=%~dp0..\scripts\add-firewall-rule.ps1"
set "SCREENBRIDGE_HOST_CONFIG=%APPDATA%\ScreenBridge\host.toml"

echo ScreenBridge Host Firewall
echo.
echo This action will ask Windows for Administrator approval.
echo It will allow inbound TCP connections for ScreenBridge Host.
echo.

powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$quote = [char]34; $script = $env:SCREENBRIDGE_FIREWALL_SCRIPT; $config = $env:SCREENBRIDGE_HOST_CONFIG; $arguments = @('-NoProfile','-ExecutionPolicy','Bypass','-File',($quote + $script + $quote),'-ConfigPath',($quote + $config + $quote),'-Profile','Any','-Pause') -join ' '; $process = Start-Process -FilePath powershell.exe -Verb RunAs -Wait -PassThru -ArgumentList $arguments; exit $process.ExitCode"
set "SCREENBRIDGE_EXIT_CODE=%ERRORLEVEL%"

echo.
echo Elevated firewall action finished with code %SCREENBRIDGE_EXIT_CODE%.
echo Press any key to close this window.
pause >nul
exit /b %SCREENBRIDGE_EXIT_CODE%
