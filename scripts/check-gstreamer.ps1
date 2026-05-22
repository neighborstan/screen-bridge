[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$script:Results = [System.Collections.Generic.List[object]]::new()

function Add-Result {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet("PASS", "WARN", "FAIL")]
        [string]$Status,

        [Parameter(Mandatory = $true)]
        [string]$Name,

        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    $script:Results.Add([pscustomobject]@{
        Status = $Status
        Name = $Name
        Message = $Message
    }) | Out-Null

    $color = switch ($Status) {
        "PASS" { "Green" }
        "WARN" { "Yellow" }
        "FAIL" { "Red" }
    }

    Write-Host ("[{0}] {1} - {2}" -f $Status, $Name, $Message) -ForegroundColor $color
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

function Get-GStreamerRootCandidates {
    $candidates = [System.Collections.Generic.List[string]]::new()
    $envNames = @(
        "GSTREAMER_1_0_ROOT_MSVC_X86_64",
        "GSTREAMER_ROOT_X86_64"
    )

    foreach ($envName in $envNames) {
        $value = [Environment]::GetEnvironmentVariable($envName)
        if (-not [string]::IsNullOrWhiteSpace($value)) {
            $candidates.Add($value.TrimEnd("\", "/")) | Out-Null
        }
    }

    $defaultRoot = "C:\gstreamer\1.0\msvc_x86_64"
    if (Test-Path -LiteralPath $defaultRoot) {
        $candidates.Add($defaultRoot) | Out-Null
    }

    return $candidates | Select-Object -Unique
}

function Get-GStreamerPathHint {
    $roots = Get-GStreamerRootCandidates
    foreach ($root in $roots) {
        $bin = Join-Path -Path $root -ChildPath "bin"
        if (Test-Path -LiteralPath $bin) {
            return "Add $bin to PATH and restart PowerShell."
        }
    }

    return "Install the 64-bit MSVC Runtime and Development packages from the official GStreamer download page."
}

function Get-PkgConfigHint {
    $roots = Get-GStreamerRootCandidates
    foreach ($root in $roots) {
        $pkgConfigPath = Join-Path -Path $root -ChildPath "lib\pkgconfig"
        if (Test-Path -LiteralPath $pkgConfigPath) {
            return "Set PKG_CONFIG_PATH to include $pkgConfigPath."
        }
    }

    return "Install the GStreamer Development package; it contains pkg-config files."
}

function Check-Rust {
    $rustc = Find-Application -Names @("rustc.exe", "rustc")
    if ($null -eq $rustc) {
        Add-Result -Status "FAIL" -Name "Rust rustc" -Message "rustc was not found in PATH."
        return
    }

    $rustcVersion = Invoke-Application -Path $rustc -Arguments @("-Vv")
    if ($rustcVersion.ExitCode -ne 0) {
        Add-Result -Status "FAIL" -Name "Rust rustc" -Message "rustc -Vv failed: $($rustcVersion.Output)"
        return
    }

    $hostLine = ($rustcVersion.Output -split "`r?`n") | Where-Object { $_ -like "host:*" } | Select-Object -First 1
    if ($hostLine -notmatch "x86_64-pc-windows-msvc") {
        Add-Result -Status "FAIL" -Name "Rust target" -Message "Expected x86_64-pc-windows-msvc, got $hostLine."
    } else {
        $releaseLine = ($rustcVersion.Output -split "`r?`n") | Where-Object { $_ -like "release:*" } | Select-Object -First 1
        Add-Result -Status "PASS" -Name "Rust rustc" -Message "$releaseLine, $hostLine"
    }

    $cargo = Find-Application -Names @("cargo.exe", "cargo")
    if ($null -eq $cargo) {
        Add-Result -Status "FAIL" -Name "Rust cargo" -Message "cargo was not found in PATH."
        return
    }

    $cargoVersion = Invoke-Application -Path $cargo -Arguments @("-V")
    if ($cargoVersion.ExitCode -eq 0) {
        Add-Result -Status "PASS" -Name "Rust cargo" -Message $cargoVersion.Output
    } else {
        Add-Result -Status "FAIL" -Name "Rust cargo" -Message "cargo -V failed: $($cargoVersion.Output)"
    }
}

function Check-GStreamerPath {
    $gstLaunch = Find-Application -Names @("gst-launch-1.0.exe", "gst-launch-1.0")
    if ($null -eq $gstLaunch) {
        Add-Result -Status "FAIL" -Name "GStreamer gst-launch" -Message "gst-launch-1.0.exe was not found in PATH. $(Get-GStreamerPathHint)"
    } else {
        Add-Result -Status "PASS" -Name "GStreamer gst-launch" -Message $gstLaunch
    }

    $gstInspect = Find-Application -Names @("gst-inspect-1.0.exe", "gst-inspect-1.0")
    if ($null -eq $gstInspect) {
        Add-Result -Status "FAIL" -Name "GStreamer gst-inspect" -Message "gst-inspect-1.0.exe was not found in PATH. $(Get-GStreamerPathHint)"
        return $null
    }

    $version = Invoke-Application -Path $gstInspect -Arguments @("--version")
    if ($version.ExitCode -eq 0) {
        $versionLine = ($version.Output -split "`r?`n") | Select-Object -First 1
        Add-Result -Status "PASS" -Name "GStreamer version" -Message $versionLine
    } else {
        Add-Result -Status "FAIL" -Name "GStreamer version" -Message "gst-inspect-1.0 --version failed: $($version.Output)"
    }

    return $gstInspect
}

function Check-PkgConfig {
    $pkgConfig = Find-Application -Names @("pkg-config.exe", "pkg-config")
    if ($null -eq $pkgConfig) {
        Add-Result -Status "FAIL" -Name "pkg-config" -Message "pkg-config was not found in PATH. $(Get-GStreamerPathHint)"
        return
    }

    Add-Result -Status "PASS" -Name "pkg-config" -Message $pkgConfig

    $packages = @("gstreamer-1.0", "gstreamer-rtsp-server-1.0")
    $versions = @{}

    foreach ($package in $packages) {
        $result = Invoke-Application -Path $pkgConfig -Arguments @("--modversion", $package)
        if ($result.ExitCode -eq 0 -and -not [string]::IsNullOrWhiteSpace($result.Output)) {
            $versions[$package] = $result.Output
            Add-Result -Status "PASS" -Name "pkg-config $package" -Message $result.Output
        } else {
            Add-Result -Status "FAIL" -Name "pkg-config $package" -Message "Package was not found. $(Get-PkgConfigHint)"
        }
    }

    if ($versions.ContainsKey("gstreamer-1.0") -and $versions.ContainsKey("gstreamer-rtsp-server-1.0")) {
        if ($versions["gstreamer-1.0"] -eq $versions["gstreamer-rtsp-server-1.0"]) {
            Add-Result -Status "PASS" -Name "GStreamer package versions" -Message "Core and RTSP server versions match: $($versions["gstreamer-1.0"])"
        } else {
            Add-Result -Status "WARN" -Name "GStreamer package versions" -Message "Core is $($versions["gstreamer-1.0"]), RTSP server is $($versions["gstreamer-rtsp-server-1.0"]). Use matching Runtime and Development installers."
        }
    }
}

function Check-Elements {
    param(
        [AllowNull()]
        [string]$GstInspect
    )

    if ([string]::IsNullOrWhiteSpace($GstInspect)) {
        Add-Result -Status "FAIL" -Name "GStreamer elements" -Message "Skipped because gst-inspect-1.0.exe is not available."
        return
    }

    $mandatory = @(
        "d3d11screencapturesrc",
        "d3d11convert",
        "d3d11download",
        "d3d11videosink",
        "videoconvert",
        "mfh264enc",
        "x264enc",
        "rtph264pay",
        "rtph264depay",
        "h264parse",
        "decodebin",
        "rtspsrc"
    )

    foreach ($element in $mandatory) {
        $result = Invoke-Application -Path $GstInspect -Arguments @($element)
        if ($result.ExitCode -eq 0) {
            Add-Result -Status "PASS" -Name "GStreamer element $element" -Message "available"
        } else {
            Add-Result -Status "FAIL" -Name "GStreamer element $element" -Message "missing"
        }
    }

    $optionalEncoders = @(
        "nvd3d11h264enc",
        "nvautogpuh264enc",
        "qsvh264enc",
        "amfh264enc"
    )

    $foundOptional = [System.Collections.Generic.List[string]]::new()
    foreach ($encoder in $optionalEncoders) {
        $result = Invoke-Application -Path $GstInspect -Arguments @($encoder)
        if ($result.ExitCode -eq 0) {
            $foundOptional.Add($encoder) | Out-Null
        }
    }

    if ($foundOptional.Count -gt 0) {
        Add-Result -Status "PASS" -Name "Optional hardware encoders" -Message ($foundOptional -join ", ")
    } else {
        Add-Result -Status "WARN" -Name "Optional hardware encoders" -Message "No vendor H.264 encoder was found; software fallback can still work through x264enc."
    }
}

function Check-Network {
    $addresses = [System.Collections.Generic.List[string]]::new()
    $interfaces = [System.Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces()

    foreach ($interface in $interfaces) {
        if ($interface.OperationalStatus -ne [System.Net.NetworkInformation.OperationalStatus]::Up) {
            continue
        }

        if (
            $interface.NetworkInterfaceType -eq [System.Net.NetworkInformation.NetworkInterfaceType]::Loopback -or
            $interface.NetworkInterfaceType -eq [System.Net.NetworkInformation.NetworkInterfaceType]::Tunnel
        ) {
            continue
        }

        foreach ($unicast in $interface.GetIPProperties().UnicastAddresses) {
            if ($unicast.Address.AddressFamily -ne [System.Net.Sockets.AddressFamily]::InterNetwork) {
                continue
            }

            $ip = $unicast.Address.ToString()
            if ($ip.StartsWith("169.254.")) {
                continue
            }

            $addresses.Add(("{0}: {1}" -f $interface.Name, $ip)) | Out-Null
        }
    }

    if ($addresses.Count -gt 0) {
        Add-Result -Status "PASS" -Name "Local IPv4 interfaces" -Message ($addresses -join "; ")
    } else {
        Add-Result -Status "WARN" -Name "Local IPv4 interfaces" -Message "No non-loopback IPv4 address was found. Check network adapter state before LAN smoke tests."
    }
}

Write-Host "ScreenBridge environment check"
Write-Host "Working directory: $((Get-Location).Path)"
Write-Host ""

Check-Rust
$gstInspectPath = Check-GStreamerPath
Check-PkgConfig
Check-Elements -GstInspect $gstInspectPath
Check-Network

$failed = @($script:Results | Where-Object { $_.Status -eq "FAIL" }).Count
$warnings = @($script:Results | Where-Object { $_.Status -eq "WARN" }).Count
$passed = @($script:Results | Where-Object { $_.Status -eq "PASS" }).Count

Write-Host ""
Write-Host ("Summary: {0} passed, {1} warning(s), {2} failed" -f $passed, $warnings, $failed)

if ($failed -gt 0) {
    Write-Host "Environment is not ready for ScreenBridge GStreamer work." -ForegroundColor Red
    exit 1
}

Write-Host "Environment is ready for ScreenBridge GStreamer work." -ForegroundColor Green
exit 0
