[CmdletBinding()]
param(
    [string]$GStreamerRoot = "",
    [string]$InnoSetupCompiler = "",
    [switch]$NoInstallMissingTools,
    [switch]$SkipCargoBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-ProjectRoot {
    $scriptDirectory = Split-Path -Parent $PSCommandPath
    return (Resolve-Path -LiteralPath (Join-Path -Path $scriptDirectory -ChildPath "..")).Path
}

function Resolve-Iscc {
    param(
        [string]$RequestedPath,
        [switch]$NoInstallMissingTools
    )

    if (-not [string]::IsNullOrWhiteSpace($RequestedPath)) {
        $resolved = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($RequestedPath)
        if (Test-Path -LiteralPath $resolved -PathType Leaf) {
            return $resolved
        }

        Stop-MissingInnoSetup -RequestedPath $RequestedPath -NoInstallMissingTools:$true
    }

    $existing = Find-Iscc
    if (-not [string]::IsNullOrWhiteSpace($existing)) {
        return $existing
    }

    if ($NoInstallMissingTools) {
        Stop-MissingInnoSetup -NoInstallMissingTools:$true
    }

    Install-InnoSetupWithWinget

    $installed = Find-Iscc
    if (-not [string]::IsNullOrWhiteSpace($installed)) {
        return $installed
    }

    Stop-MissingInnoSetup
}

function Find-Iscc {
    $command = Get-Command "iscc.exe" -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        return $command.Source
    }

    $candidates = @(
        (Join-Path -Path ${env:ProgramFiles(x86)} -ChildPath "Inno Setup 6\ISCC.exe"),
        (Join-Path -Path $env:ProgramFiles -ChildPath "Inno Setup 6\ISCC.exe"),
        (Join-Path -Path $env:LocalAppData -ChildPath "Programs\Inno Setup 6\ISCC.exe")
    )

    foreach ($candidate in $candidates) {
        if (-not [string]::IsNullOrWhiteSpace($candidate) -and (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            return $candidate
        }
    }

    return $null
}

function Install-InnoSetupWithWinget {
    Write-Host ""
    Write-Host "Inno Setup compiler ISCC.exe was not found."
    Write-Host "ScreenBridge can install Inno Setup 6 with winget and then continue building the installer."
    Write-Host "Command:"
    Write-Host "  winget install --id JRSoftware.InnoSetup -e -s winget --accept-source-agreements --accept-package-agreements --silent"
    Write-Host ""

    $winget = Get-Command "winget.exe" -ErrorAction SilentlyContinue
    if ($null -eq $winget) {
        Stop-MissingInnoSetup -WingetMissing
    }

    $answer = Read-Host "Install Inno Setup 6 now? [Y/n]"
    $normalized = $answer.Trim().ToLowerInvariant()
    $acceptedAnswers = @("", "y", "yes", "д", "да")
    $accepted = $acceptedAnswers -contains $normalized

    if (-not $accepted) {
        Stop-MissingInnoSetup
    }

    $arguments = @(
        "install",
        "--id",
        "JRSoftware.InnoSetup",
        "-e",
        "-s",
        "winget",
        "--accept-source-agreements",
        "--accept-package-agreements",
        "--silent"
    )

    try {
        $exitCode = Invoke-WingetInstallInnoSetup -WingetPath $winget.Source -Arguments $arguments
    } catch {
        $installed = Find-Iscc
        if (-not [string]::IsNullOrWhiteSpace($installed)) {
            Write-Host "winget reported an error, but ISCC.exe was found at $installed."
            Write-Host "Continuing installer build."
            return
        }

        throw
    }

    $installed = Find-Iscc
    if ($exitCode -ne 0) {
        if (-not [string]::IsNullOrWhiteSpace($installed)) {
            Write-Host "winget returned exit code $exitCode, but ISCC.exe was found at $installed."
            Write-Host "Continuing installer build."
            return
        }

        throw "winget failed to install Inno Setup 6. Exit code: $exitCode."
    }

    if ([string]::IsNullOrWhiteSpace($installed)) {
        Stop-MissingInnoSetup
    }

    Write-Host "Inno Setup compiler found at $installed."
}

function Invoke-WingetInstallInnoSetup {
    param(
        [string]$WingetPath,
        [string[]]$Arguments
    )

    $nativePreference = Get-Variable -Name "PSNativeCommandUseErrorActionPreference" -ErrorAction SilentlyContinue
    if ($null -ne $nativePreference) {
        $previousNativePreference = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
    }

    try {
        & $WingetPath @Arguments
        return $LASTEXITCODE
    } finally {
        if ($null -ne $nativePreference) {
            $PSNativeCommandUseErrorActionPreference = $previousNativePreference
        }
    }
}

function Stop-MissingInnoSetup {
    param(
        [string]$RequestedPath = "",
        [switch]$NoInstallMissingTools,
        [switch]$WingetMissing
    )

    Write-Host ""

    if (-not [string]::IsNullOrWhiteSpace($RequestedPath)) {
        Write-Host "Inno Setup compiler was not found at $RequestedPath."
    } else {
        Write-Host "Inno Setup compiler ISCC.exe was not found."
    }

    Write-Host "ScreenBridge installer cannot be produced until Inno Setup 6 is installed on this build machine."
    if ($NoInstallMissingTools) {
        Write-Host "Automatic installation is disabled by -NoInstallMissingTools."
    } elseif ($WingetMissing) {
        Write-Host "winget.exe was not found, so automatic installation is unavailable."
    }
    Write-Host "Install Inno Setup 6 with:"
    Write-Host "  winget install --id JRSoftware.InnoSetup -e -s winget --accept-source-agreements --accept-package-agreements --silent"
    Write-Host "Or download it from:"
    Write-Host "  https://jrsoftware.org/isdl.php"
    Write-Host "Then reopen PowerShell and run:"
    Write-Host "  .\scripts\build-installer.ps1"
    Write-Host "If ISCC.exe is installed in a custom directory, pass:"
    Write-Host "  .\scripts\build-installer.ps1 -InnoSetupCompiler ""C:\Path\To\ISCC.exe"""

    throw "Install Inno Setup 6 or pass -InnoSetupCompiler."
}

function Read-WorkspaceVersion {
    param(
        [string]$CargoTomlPath
    )

    $inWorkspacePackage = $false
    foreach ($line in Get-Content -Path $CargoTomlPath) {
        if ($line -match "^\[workspace\.package\]") {
            $inWorkspacePackage = $true
            continue
        }

        if ($inWorkspacePackage -and $line -match "^\[") {
            break
        }

        if ($inWorkspacePackage -and $line -match "^\s*version\s*=\s*""([^""]+)""") {
            return $Matches[1]
        }
    }

    throw "workspace.package.version was not found in Cargo.toml."
}

function Assert-RequiredPath {
    param(
        [string]$Path,
        [string]$Description
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "$Description was not found at $Path."
    }
}

$projectRoot = Resolve-ProjectRoot
$envScript = Join-Path -Path $projectRoot -ChildPath "scripts\env-gstreamer.ps1"

. $envScript -Root $GStreamerRoot

if (-not $SkipCargoBuild) {
    cargo build --workspace --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --workspace --release failed."
    }
}

$gstreamerRoot = $env:GSTREAMER_1_0_ROOT_MSVC_X86_64
$version = Read-WorkspaceVersion -CargoTomlPath (Join-Path -Path $projectRoot -ChildPath "Cargo.toml")

Assert-RequiredPath -Path (Join-Path -Path $projectRoot -ChildPath "target\release\screen-bridge-host.exe") -Description "Host release executable"
Assert-RequiredPath -Path (Join-Path -Path $projectRoot -ChildPath "target\release\screen-bridge-viewer.exe") -Description "Viewer release executable"
Assert-RequiredPath -Path (Join-Path -Path $gstreamerRoot -ChildPath "bin\gstreamer-1.0-0.dll") -Description "GStreamer core DLL"
Assert-RequiredPath -Path (Join-Path -Path $gstreamerRoot -ChildPath "lib\gstreamer-1.0") -Description "GStreamer plugin directory"
Assert-RequiredPath -Path (Join-Path -Path $gstreamerRoot -ChildPath "libexec\gstreamer-1.0\gst-plugin-scanner.exe") -Description "GStreamer plugin scanner"

$distDirectory = Join-Path -Path $projectRoot -ChildPath "dist"
New-Item -ItemType Directory -Force -Path $distDirectory | Out-Null

$iscc = Resolve-Iscc -RequestedPath $InnoSetupCompiler -NoInstallMissingTools:$NoInstallMissingTools
$installerScript = Join-Path -Path $projectRoot -ChildPath "installer\screenbridge.iss"
$arguments = @(
    "/DProjectRoot=$projectRoot",
    "/DGStreamerRoot=$gstreamerRoot",
    "/DAppVersion=$version",
    $installerScript
)

& $iscc @arguments
if ($LASTEXITCODE -ne 0) {
    throw "Inno Setup compiler failed."
}

Write-Host "Installer artifacts were written to $distDirectory."
