[CmdletBinding()]
param(
    [string]$Address = "0.0.0.0",

    [ValidateRange(1, 65535)]
    [int]$Port = 8554,

    [string]$Path = "/screen",

    [ValidateRange(-1, 2147483647)]
    [int]$MonitorIndex = -1,

    [ValidateSet("dxgi", "wgc")]
    [string]$CaptureApi = "dxgi",

    [switch]$NoCursor,

    [ValidateRange(64, 8192)]
    [int]$Width = 1280,

    [ValidateRange(64, 8192)]
    [int]$Height = 720,

    [ValidateRange(1, 240)]
    [int]$Fps = 15,

    [ValidateRange(1, 4194303)]
    [int]$BitrateKbps = 2500,

    [ValidateRange(0, 3600)]
    [int]$DurationSeconds = 0,

    [string]$GStreamerRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
    $scriptDirectory = Split-Path -Parent $PSCommandPath
    return (Resolve-Path -LiteralPath (Join-Path -Path $scriptDirectory -ChildPath "..")).Path
}

function Add-PathEntry {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Entry
    )

    $parts = $env:Path -split [System.IO.Path]::PathSeparator
    if ($parts -notcontains $Entry) {
        $env:Path = "$Entry$([System.IO.Path]::PathSeparator)$env:Path"
    }
}

function Resolve-GStreamerRoot {
    param(
        [string]$RequestedRoot
    )

    $candidates = [System.Collections.Generic.List[string]]::new()

    if (-not [string]::IsNullOrWhiteSpace($RequestedRoot)) {
        $candidates.Add($RequestedRoot) | Out-Null
    }

    $projectRoot = Resolve-ProjectRoot
    $candidates.Add((Join-Path -Path $projectRoot -ChildPath ".local\gstreamer\msvc_x86_64")) | Out-Null
    $candidates.Add((Join-Path -Path $projectRoot -ChildPath ".local\gstreamer\1.0\msvc_x86_64")) | Out-Null

    $envRoot = [Environment]::GetEnvironmentVariable("GSTREAMER_1_0_ROOT_MSVC_X86_64")
    if (-not [string]::IsNullOrWhiteSpace($envRoot)) {
        $candidates.Add($envRoot) | Out-Null
    }

    $candidates.Add("C:\gstreamer\1.0\msvc_x86_64") | Out-Null
    $candidates.Add("C:\Program Files\gstreamer\1.0\msvc_x86_64") | Out-Null

    foreach ($candidate in $candidates) {
        if ([string]::IsNullOrWhiteSpace($candidate)) {
            continue
        }

        $resolved = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($candidate)
        $gstLaunch = Join-Path -Path $resolved -ChildPath "bin\gst-launch-1.0.exe"
        if (Test-Path -LiteralPath $gstLaunch) {
            return $resolved
        }
    }

    throw "GStreamer MSVC x86_64 was not found. Pass -GStreamerRoot or run scripts/check-gstreamer.ps1 for details."
}

if (-not $Path.StartsWith("/")) {
    throw "-Path must start with /."
}

$projectRoot = Resolve-ProjectRoot
$gstreamerRoot = Resolve-GStreamerRoot -RequestedRoot $GStreamerRoot
$gstreamerBin = Join-Path -Path $gstreamerRoot -ChildPath "bin"
$pkgConfigPath = Join-Path -Path $gstreamerRoot -ChildPath "lib\pkgconfig"

$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = $gstreamerRoot
$env:GSTREAMER_ROOT_X86_64 = $gstreamerRoot
$env:PKG_CONFIG_PATH = $pkgConfigPath
Add-PathEntry -Entry $gstreamerBin

$arguments = [System.Collections.Generic.List[string]]::new()
$arguments.Add("run") | Out-Null
$arguments.Add("-p") | Out-Null
$arguments.Add("screen-bridge-host") | Out-Null
$arguments.Add("--example") | Out-Null
$arguments.Add("rtsp-smoke") | Out-Null
$arguments.Add("--") | Out-Null
$arguments.Add("--address") | Out-Null
$arguments.Add($Address) | Out-Null
$arguments.Add("--port") | Out-Null
$arguments.Add($Port.ToString()) | Out-Null
$arguments.Add("--path") | Out-Null
$arguments.Add($Path) | Out-Null
$arguments.Add("--monitor-index") | Out-Null
$arguments.Add($MonitorIndex.ToString()) | Out-Null
$arguments.Add("--capture-api") | Out-Null
$arguments.Add($CaptureApi) | Out-Null
$arguments.Add("--width") | Out-Null
$arguments.Add($Width.ToString()) | Out-Null
$arguments.Add("--height") | Out-Null
$arguments.Add($Height.ToString()) | Out-Null
$arguments.Add("--fps") | Out-Null
$arguments.Add($Fps.ToString()) | Out-Null
$arguments.Add("--bitrate-kbps") | Out-Null
$arguments.Add($BitrateKbps.ToString()) | Out-Null
$arguments.Add("--duration-seconds") | Out-Null
$arguments.Add($DurationSeconds.ToString()) | Out-Null

if ($NoCursor.IsPresent) {
    $arguments.Add("--no-cursor") | Out-Null
}

Write-Host "Starting RTSP smoke server."
Write-Host "GStreamer root: $gstreamerRoot"
Write-Host "Local URL: rtsp://127.0.0.1:$Port$Path"
Write-Host "LAN URL: rtsp://<host-ip>:$Port$Path"
Write-Host "Use TCP transport in VLC."

Set-Location -LiteralPath $projectRoot
cargo @arguments
