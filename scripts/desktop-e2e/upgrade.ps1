[CmdletBinding()]
param(
    [string]$RunId = "",
    [Parameter(Mandatory = $true)][string]$OldSetup,
    [Parameter(Mandatory = $true)][string]$NewSetup,
    [string]$InstallDir = "",
    [string]$StateHome = "",
    [int]$ColdStartTimeoutSeconds = 120,
    [int]$ShutdownTimeoutSeconds = 30,
    [int]$HealthTimeoutSeconds = 55,
    [int]$SessionTimeoutSeconds = 90
)

# U1 install-upgrade-install data preservation run.
#
# Phase A: install $OldSetup into a throwaway tree, cold start, create a
# workspace marker through the local API, graceful close.
# Phase B: install $NewSetup on top of the same tree, cold start on the same
# STATE_HOME, assert the marker survives and the bundled parser tree exists.
#
# Reuses l0.ps1 functions via dot-source (-FunctionsOnly); same safety
# contract: graceful CloseMainWindow only, never Stop-Process. AppData files
# are backed up before launch and restored in finally (KD12: STATE_HOME does
# not cover Tauri AppData).
#
# Session note: old installers may carry the pre-fix bootstrap race (register
# fired before migrations land, no frontend retry — fixed in 559c042d). When
# the app does not bootstrap a valid session in time, the harness falls back
# to driving /api/auth/register|login directly with the local_user.json creds
# and persists local_session.json in the shell's own shape. The state dir's
# jwt.secret survives the upgrade, so the phase-A token stays valid in phase B.

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($RunId)) {
    $RunId = [guid]::NewGuid().ToString("N").Substring(0, 12)
}
$UpgradeRoot = Join-Path $env:TEMP "cos-e2e-upgrade-$RunId"
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $UpgradeRoot "app"
}
if ([string]::IsNullOrWhiteSpace($StateHome)) {
    $StateHome = Join-Path $UpgradeRoot "state"
}
$AppDataBackup = Join-Path $UpgradeRoot "appdata-backup"

. (Join-Path $PSScriptRoot "l0.ps1") -FunctionsOnly `
    -RunId $RunId -InstallDir $InstallDir -StateHome $StateHome `
    -AppDataBackupPath $AppDataBackup `
    -ColdStartTimeoutSeconds $ColdStartTimeoutSeconds `
    -ShutdownTimeoutSeconds $ShutdownTimeoutSeconds `
    -HealthTimeoutSeconds $HealthTimeoutSeconds `
    -SessionTimeoutSeconds $SessionTimeoutSeconds

# Start-E2eApp takes its env-provisioning branch when $KeepRunning is set
# (E2E_ENABLED + BYOK key); CDP is not used by this run.
$KeepRunning = $true

$script:SessionToken = ""

function Install-Setup {
    param([Parameter(Mandatory = $true)][string]$SetupExe, [Parameter(Mandatory = $true)][string]$DestDir)
    # NSIS: /D must be the last argument and is never quoted. Our paths carry no spaces.
    $p = Start-Process -FilePath $SetupExe -ArgumentList '/S', "/D=$DestDir" -Wait -PassThru
    if ($p.ExitCode -ne 0) {
        Add-Signal FAIL "S-desktop-upgrade-install" "setup $SetupExe exited with $($p.ExitCode)"
        return $false
    }
    return $true
}

function Invoke-AuthApi {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Body
    )
    $json = $Body | ConvertTo-Json -Compress
    return Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:18080$Path" `
        -Body $json -ContentType "application/json" -TimeoutSec 10
}

function Seed-LocalSessionViaApi {
    # Harness fallback for builds whose frontend never retries bootstrap.
    $appDataDir = Join-Path $env:APPDATA "com.contextos.desktop"
    $credsPath = Join-Path $appDataDir "local_user.json"
    New-Item -ItemType Directory -Force -Path $appDataDir | Out-Null
    $creds = $null
    if (Test-Path $credsPath) {
        try { $creds = Get-Content -Raw -Path $credsPath | ConvertFrom-Json } catch { $creds = $null }
    }
    if ($null -eq $creds -or [string]::IsNullOrWhiteSpace([string]$creds.password) -or $creds.password.Length -lt 8) {
        $creds = [pscustomobject]@{
            email = "local@context-os.client"
            password = ([guid]::NewGuid().ToString("N") + [guid]::NewGuid().ToString("N"))
        }
        $creds | ConvertTo-Json | Set-Content -Encoding UTF8 -Path $credsPath
    }
    $deadline = (Get-Date).AddSeconds($SessionTimeoutSeconds)
    $lastError = ""
    while ((Get-Date) -lt $deadline) {
        try {
            $resp = Invoke-AuthApi "/api/auth/register" @{
                email = [string]$creds.email
                password = [string]$creds.password
                full_name = "Local User"
                terms_version = "2026-06-13"
                privacy_version = "2026-06-13"
                local = $true
            }
        } catch {
            $lastError = "register: $($_.Exception.Message)"
            try {
                $resp = Invoke-AuthApi "/api/auth/login" @{
                    email = [string]$creds.email
                    password = [string]$creds.password
                }
            } catch {
                $lastError = "$lastError | login: $($_.Exception.Message)"
                Start-Sleep -Seconds 2
                continue
            }
        }
        $token = [string]$resp.data.token
        if (-not [string]::IsNullOrWhiteSpace($token)) {
            $session = [ordered]@{
                token = $token
                user = [ordered]@{
                    id = [string]$resp.data.user.id
                    email = [string]$resp.data.user.email
                    full_name = [string]$resp.data.user.full_name
                }
            }
            $session | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 -Path (Join-Path $appDataDir "local_session.json")
            $script:SessionToken = $token
            return $true
        }
        Start-Sleep -Seconds 2
    }
    if (-not [string]::IsNullOrWhiteSpace($lastError)) {
        Add-Signal WARN "S-desktop-session-seed" "seed loop last error: $lastError"
    }
    return $false
}

function Ensure-E2eLocalSession {
    $sessionStatus = Wait-LocalSession $SessionTimeoutSeconds
    if ($null -ne $sessionStatus -and $sessionStatus.valid) {
        $script:SessionToken = ""
        return $true
    }
    if (Seed-LocalSessionViaApi) {
        return $true
    }
    Add-Signal FAIL "S-desktop-session" "local session not valid after cold start and harness-level register/login fallback failed"
    return $false
}

function Get-ApiToken {
    if (-not [string]::IsNullOrWhiteSpace($script:SessionToken)) {
        return $script:SessionToken
    }
    $path = Join-Path $env:APPDATA "com.contextos.desktop\local_session.json"
    $data = Get-Content -Raw -Path $path | ConvertFrom-Json
    $token = [string]$data.token
    if ([string]::IsNullOrWhiteSpace($token)) {
        throw "local_session.json has no token"
    }
    return $token
}

function Start-And-WaitHealthy {
    $process = Start-E2eApp
    $global:Details.app_pid = $process.Id
    if (-not (Wait-PortsOpen $ColdStartTimeoutSeconds)) {
        Add-Signal FAIL "S-desktop-port" "cold start ports did not open within ${ColdStartTimeoutSeconds}s"
        return $false
    }
    if (-not (Test-ApiHealth)) {
        Add-Signal FAIL "S-desktop-port" "local API health did not return 200 within ${HealthTimeoutSeconds}s"
        return $false
    }
    return (Ensure-E2eLocalSession)
}

function Stop-And-WaitClosed {
    if (-not (Close-AppProcesses)) {
        Add-Signal FAIL "S-desktop-no-app-window" "E2E Context-OS.exe main window cannot be closed"
        return $false
    }
    if (-not (Wait-PortsClosed $ShutdownTimeoutSeconds)) {
        Add-Signal FAIL "S-desktop-shutdown-timeout" "ports did not release within ${ShutdownTimeoutSeconds}s after close"
        return $false
    }
    return $true
}

function Invoke-U1 {
    foreach ($port in @(5433, 6380, 18080)) {
        if ((Test-PortOwner $port "api") -ne "free") {
            Add-Signal FAIL "S-desktop-port-owner" "port $port is busy before the upgrade run; refusing to touch it"
            Write-Result
            return
        }
    }

    if (-not (Install-Setup $OldSetup $InstallDir)) { Write-Result; return }
    if (-not (Test-InstallTree)) { Write-Result; return }

    # ---- Phase A: old version seeds data ----
    Backup-AppDataFiles
    if (-not (Start-And-WaitHealthy)) { Write-Result; return }
    $marker = "u1-upgrade-marker-$RunId"
    try {
        $headers = @{ Authorization = "Bearer $(Get-ApiToken)" }
        $body = @{ name = $marker; description = "u1 upgrade marker" } | ConvertTo-Json
        $created = Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:18080/api/v1/workspaces" `
            -Headers $headers -Body $body -ContentType "application/json"
        $global:Details.marker_workspace_id = [string]$created.workspace.id
    } catch {
        Add-Signal FAIL "S-desktop-upgrade-marker" "failed to create pre-upgrade workspace marker: $($_.Exception.Message)"
        Write-Result
        return
    }
    if (-not (Stop-And-WaitClosed)) { Write-Result; return }

    # ---- Upgrade install on top ----
    if (-not (Install-Setup $NewSetup $InstallDir)) { Write-Result; return }

    # ---- Phase B: new version must see the marker ----
    if (-not (Start-And-WaitHealthy)) { Write-Result; return }

    $litPath = Join-Path $InstallDir "runtime\parsers\lit\lit.exe"
    $global:Details.upgrade_lit_present = Test-Path $litPath

    $envPath = Join-Path $StateHome "client.env"
    if (Test-Path $envPath) {
        $envMap = Read-EnvFile $envPath
        $global:Details.upgrade_liteparse_bin = if ($envMap.Contains("LITEPARSE_BIN")) { $envMap["LITEPARSE_BIN"] } else { "" }
    }

    try {
        $headers = @{ Authorization = "Bearer $(Get-ApiToken)" }
        $list = Invoke-RestMethod -Method Get -Uri "http://127.0.0.1:18080/api/v1/workspaces" -Headers $headers
        $names = @()
        if ($null -ne $list -and $null -ne $list.workspaces) {
            $names = @($list.workspaces | ForEach-Object { [string]$_.name })
        }
        $global:Details.workspaces_after_upgrade = $names
        if ($names -notcontains $marker) {
            Add-Signal FAIL "S-desktop-upgrade-data" "workspace marker '$marker' missing after upgrade (have: $($names -join ', '))"
            Write-Result
            return
        }
    } catch {
        Add-Signal FAIL "S-desktop-upgrade-data" "failed to list workspaces after upgrade: $($_.Exception.Message)"
        Write-Result
        return
    }

    if (-not (Stop-And-WaitClosed)) { Write-Result; return }
    $global:Details.teardown_ok = $true
    Write-Result
}

try {
    Invoke-U1
} catch {
    Add-Signal FAIL "S-desktop-script" "unhandled U1 error: $($_.Exception.Message)"
    Write-Result
} finally {
    # Never leak a test-tree app process; always put the user's AppData back.
    try {
        if (@(Get-InstallAppProcesses).Count -gt 0) {
            Close-AppProcesses | Out-Null
            Wait-PortsClosed $ShutdownTimeoutSeconds | Out-Null
        }
    } catch {
    }
    try {
        Restore-AppDataFiles
    } catch {
    }
}

if ($global:Ok) {
    exit 0
}
exit 1
