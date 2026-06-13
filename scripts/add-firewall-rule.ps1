[CmdletBinding()]
param(
    [int]$Port = 0,
    [string]$ConfigPath = "",
    [string]$DisplayName = "ScreenBridge Host RTSP",
    [ValidateSet("Any", "Domain", "Private", "Public")]
    [string]$Profile = "Any",
    [switch]$Pause
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Read-ConfigPort {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $null
    }

    foreach ($line in Get-Content -LiteralPath $Path) {
        if ($line -match "^\s*port\s*=\s*(\d+)\s*(#.*)?$") {
            return [int]$Matches[1]
        }
    }

    return $null
}

function Read-ConfigBindIp {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return ""
    }

    foreach ($line in Get-Content -LiteralPath $Path) {
        if ($line -match "^\s*bind_ip\s*=\s*""([^""]+)""\s*(#.*)?$") {
            return $Matches[1]
        }
    }

    return ""
}

function Resolve-RulePort {
    param(
        [int]$RequestedPort,
        [string]$HostConfigPath
    )

    if ($RequestedPort -gt 0) {
        return $RequestedPort
    }

    $configPort = Read-ConfigPort -Path $HostConfigPath
    if ($null -ne $configPort -and $configPort -gt 0) {
        return $configPort
    }

    Write-Warning "Host config port was not found. Falling back to default TCP port 8554."
    return 8554
}

function Assert-ValidPort {
    param([int]$Value)

    if ($Value -lt 1 -or $Value -gt 65535) {
        throw "TCP port must be between 1 and 65535. Got $Value."
    }
}

function Ensure-ScreenBridgeFirewallRule {
    param(
        [string]$RuleDisplayName,
        [int]$RulePort,
        [string]$RuleProfile
    )

    $rules = @(Get-NetFirewallRule -DisplayName $RuleDisplayName -ErrorAction SilentlyContinue)
    if ($rules.Count -gt 1) {
        Write-Warning "Multiple firewall rules named ""$RuleDisplayName"" were found. Updating the first rule only."
    }

    $rule = $rules | Select-Object -First 1
    if ($null -eq $rule) {
        $newRuleParams = @{
            DisplayName = $RuleDisplayName
            Direction = "Inbound"
            Action = "Allow"
            Protocol = "TCP"
            LocalPort = $RulePort
            Profile = $RuleProfile
        }
        New-NetFirewallRule @newRuleParams | Out-Null
        return "created"
    }

    if ($rule.Direction -ne "Inbound") {
        throw "Existing firewall rule ""$RuleDisplayName"" is not inbound. Use another -DisplayName or fix the existing rule manually."
    }

    $setRuleParams = @{
        Name = $rule.Name
        Enabled = "True"
        Action = "Allow"
        Profile = $RuleProfile
    }
    Set-NetFirewallRule @setRuleParams | Out-Null

    $setPortParams = @{
        Protocol = "TCP"
        LocalPort = $RulePort
    }
    $rule | Get-NetFirewallPortFilter | Set-NetFirewallPortFilter @setPortParams | Out-Null

    return "updated"
}

function Complete-Script {
    param([int]$ExitCode)

    if ($Pause) {
        Write-Host ""
        Read-Host "Press Enter to close this window" | Out-Null
    }

    exit $ExitCode
}

try {
    if (-not (Test-IsAdministrator)) {
        throw "Administrator rights are required. Reopen PowerShell as Administrator or use the installed ""ScreenBridge Allow Host Firewall"" shortcut."
    }

    $resolvedPort = Resolve-RulePort -RequestedPort $Port -HostConfigPath $ConfigPath
    Assert-ValidPort -Value $resolvedPort

    $ensureRuleParams = @{
        RuleDisplayName = $DisplayName
        RulePort = $resolvedPort
        RuleProfile = $Profile
    }
    $action = Ensure-ScreenBridgeFirewallRule @ensureRuleParams

    Write-Host "ScreenBridge firewall rule is ready."
    Write-Host "Action: $action"
    Write-Host "Rule: $DisplayName"
    Write-Host "Direction: Inbound"
    Write-Host "Protocol: TCP"
    Write-Host "Local port: $resolvedPort"
    Write-Host "Profile: $Profile"

    $bindIp = Read-ConfigBindIp -Path $ConfigPath
    if ([string]::IsNullOrWhiteSpace($bindIp)) {
        $bindIp = "<host-ip shown by ScreenBridge Host Bind line>"
    }

    Write-Host ""
    Write-Host "From the viewer computer, verify:"
    Write-Host "  Test-NetConnection -ComputerName $bindIp -Port $resolvedPort"

    Complete-Script -ExitCode 0
} catch {
    Write-Error $_
    Complete-Script -ExitCode 1
}
