[CmdletBinding()]
param(
    [switch]$Encode,

    [ValidateRange(0, 3600)]
    [int]$DurationSeconds = 0,

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

function Find-Application {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Names
    )

    foreach ($name in $Names) {
        $command = Get-Command -Name $name -CommandType Application -ErrorAction SilentlyContinue |
            Select-Object -First 1

        if ($null -ne $command) {
            return $command.Source
        }
    }

    return $null
}

function Invoke-Application {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $output = & $Path @Arguments 2>&1
    $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
    $text = ($output | ForEach-Object { $_.ToString() }) -join [Environment]::NewLine

    return [pscustomobject]@{
        ExitCode = $exitCode
        Output = $text.Trim()
    }
}

function Assert-GStreamerElement {
    param(
        [Parameter(Mandatory = $true)]
        [string]$GstInspect,

        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $result = Invoke-Application -Path $GstInspect -Arguments @($Name)
    if ($result.ExitCode -ne 0) {
        throw "Required GStreamer element was not found: $Name. Run scripts/check-gstreamer.ps1."
    }
}

function Format-GstBool {
    param(
        [bool]$Value
    )

    if ($Value) {
        return "true"
    }

    return "false"
}

function New-CaptureArguments {
    param(
        [int]$BufferCount
    )

    $showCursor = Format-GstBool -Value (-not $NoCursor.IsPresent)
    $arguments = [System.Collections.Generic.List[string]]::new()
    $arguments.Add("d3d11screencapturesrc") | Out-Null
    $arguments.Add("monitor-index=$MonitorIndex") | Out-Null
    $arguments.Add("show-cursor=$showCursor") | Out-Null
    $arguments.Add("capture-api=$CaptureApi") | Out-Null

    if ($BufferCount -gt 0) {
        $arguments.Add("num-buffers=$BufferCount") | Out-Null
    }

    return $arguments
}

function New-PreviewPipelineArguments {
    param(
        [int]$BufferCount
    )

    $arguments = [System.Collections.Generic.List[string]]::new()
    $arguments.Add("-e") | Out-Null
    foreach ($argument in (New-CaptureArguments -BufferCount $BufferCount)) {
        $arguments.Add($argument) | Out-Null
    }

    $arguments.Add("!") | Out-Null
    $arguments.Add("queue") | Out-Null
    $arguments.Add("max-size-buffers=2") | Out-Null
    $arguments.Add("leaky=downstream") | Out-Null
    $arguments.Add("!") | Out-Null
    $arguments.Add("d3d11videosink") | Out-Null
    $arguments.Add("sync=false") | Out-Null

    return $arguments.ToArray()
}

function New-EncodePipelineArguments {
    param(
        [int]$BufferCount
    )

    $arguments = [System.Collections.Generic.List[string]]::new()
    $arguments.Add("-e") | Out-Null
    foreach ($argument in (New-CaptureArguments -BufferCount $BufferCount)) {
        $arguments.Add($argument) | Out-Null
    }

    $caps = "video/x-raw(memory:D3D11Memory),format=NV12,width=$Width,height=$Height,framerate=$Fps/1"

    $arguments.Add("!") | Out-Null
    $arguments.Add("queue") | Out-Null
    $arguments.Add("max-size-buffers=2") | Out-Null
    $arguments.Add("leaky=downstream") | Out-Null
    $arguments.Add("!") | Out-Null
    $arguments.Add("d3d11convert") | Out-Null
    $arguments.Add("!") | Out-Null
    $arguments.Add($caps) | Out-Null
    $arguments.Add("!") | Out-Null
    $arguments.Add("mfh264enc") | Out-Null
    $arguments.Add("bitrate=$BitrateKbps") | Out-Null
    $arguments.Add("rc-mode=cbr") | Out-Null
    $arguments.Add("low-latency=true") | Out-Null
    $arguments.Add("gop-size=$Fps") | Out-Null
    $arguments.Add("!") | Out-Null
    $arguments.Add("h264parse") | Out-Null
    $arguments.Add("config-interval=1") | Out-Null
    $arguments.Add("!") | Out-Null
    $arguments.Add("fakesink") | Out-Null
    $arguments.Add("sync=false") | Out-Null

    return $arguments.ToArray()
}

if ($Encode.IsPresent -and $DurationSeconds -eq 0) {
    $DurationSeconds = 10
}

$gstreamerRoot = Resolve-GStreamerRoot -RequestedRoot $GStreamerRoot
$gstreamerBin = Join-Path -Path $gstreamerRoot -ChildPath "bin"
$pkgConfigPath = Join-Path -Path $gstreamerRoot -ChildPath "lib\pkgconfig"

$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = $gstreamerRoot
$env:GSTREAMER_ROOT_X86_64 = $gstreamerRoot
$env:PKG_CONFIG_PATH = $pkgConfigPath
Add-PathEntry -Entry $gstreamerBin

$gstLaunch = Find-Application -Names @("gst-launch-1.0.exe", "gst-launch-1.0")
if ($null -eq $gstLaunch) {
    throw "gst-launch-1.0.exe was not found. Run scripts/check-gstreamer.ps1."
}

$gstInspect = Find-Application -Names @("gst-inspect-1.0.exe", "gst-inspect-1.0")
if ($null -eq $gstInspect) {
    throw "gst-inspect-1.0.exe was not found. Run scripts/check-gstreamer.ps1."
}

Assert-GStreamerElement -GstInspect $gstInspect -Name "d3d11screencapturesrc"
Assert-GStreamerElement -GstInspect $gstInspect -Name "queue"

if ($Encode.IsPresent) {
    Assert-GStreamerElement -GstInspect $gstInspect -Name "d3d11convert"
    Assert-GStreamerElement -GstInspect $gstInspect -Name "mfh264enc"
    Assert-GStreamerElement -GstInspect $gstInspect -Name "h264parse"
    Assert-GStreamerElement -GstInspect $gstInspect -Name "fakesink"
} else {
    Assert-GStreamerElement -GstInspect $gstInspect -Name "d3d11videosink"
}

$bufferCount = 0
if ($DurationSeconds -gt 0) {
    $bufferCount = $DurationSeconds * $Fps
}

if ($Encode.IsPresent) {
    $pipelineArguments = New-EncodePipelineArguments -BufferCount $bufferCount
    Write-Host "Starting H.264 encode smoke. Duration: $DurationSeconds second(s)."
} else {
    $pipelineArguments = New-PreviewPipelineArguments -BufferCount $bufferCount
    if ($DurationSeconds -gt 0) {
        Write-Host "Starting local capture preview. Duration: $DurationSeconds second(s)."
    } else {
        Write-Host "Starting local capture preview. Press Ctrl+C to stop."
    }
}

Write-Host "GStreamer root: $gstreamerRoot"
Write-Host "Command: gst-launch-1.0 $($pipelineArguments -join " ")"
& $gstLaunch @pipelineArguments
$exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }

if ($exitCode -ne 0) {
    throw "gst-launch-1.0 exited with code $exitCode."
}
