[CmdletBinding()]
param(
    [string]$Root = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
    $scriptDirectory = Split-Path -Parent $PSCommandPath
    return (Resolve-Path -LiteralPath (Join-Path -Path $scriptDirectory -ChildPath "..")).Path
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
        $bin = Join-Path -Path $resolved -ChildPath "bin"
        $gstLaunch = Join-Path -Path $bin -ChildPath "gst-launch-1.0.exe"

        if (Test-Path -LiteralPath $gstLaunch) {
            return $resolved
        }
    }

    throw "GStreamer MSVC x86_64 was not found. Pass -Root or install it into .local\gstreamer\1.0\msvc_x86_64."
}

function Add-PathEntry {
    param(
        [string]$Entry
    )

    $parts = $env:Path -split [System.IO.Path]::PathSeparator
    if ($parts -notcontains $Entry) {
        $env:Path = "$Entry$([System.IO.Path]::PathSeparator)$env:Path"
    }
}

function Disable-GioOptionalModules {
    $projectRoot = Resolve-ProjectRoot
    $gioModuleDir = Join-Path -Path $projectRoot -ChildPath "target\empty-gio-modules"
    New-Item -ItemType Directory -Force -Path $gioModuleDir | Out-Null

    # LAN RTSP does not need optional GIO modules, and the local GStreamer
    # bundle can print a noisy giolibproxy.dll warning while scanning them.
    $env:GIO_MODULE_DIR = $gioModuleDir
}

$gstreamerRoot = Resolve-GStreamerRoot -RequestedRoot $Root
$gstreamerBin = Join-Path -Path $gstreamerRoot -ChildPath "bin"
$pkgConfigPath = Join-Path -Path $gstreamerRoot -ChildPath "lib\pkgconfig"

$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = $gstreamerRoot
$env:GSTREAMER_ROOT_X86_64 = $gstreamerRoot
$env:PKG_CONFIG_PATH = $pkgConfigPath
Add-PathEntry -Entry $gstreamerBin
Disable-GioOptionalModules

Write-Host "GStreamer environment is active for this PowerShell process."
Write-Host "Root: $gstreamerRoot"
