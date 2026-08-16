[CmdletBinding()]
param(
    [string]$RunId = "",
    [string]$InstallDir = "",
    [string]$StateHome = "",
    [switch]$UseDefaultTree,
    [switch]$AuditOnly,
    [switch]$KeepRunning,
    [switch]$TeardownOnly,
    # l3 cloud-login acceptance: no gate bypass env, no legacy BYOK seed.
    [switch]$NoCloudGateBypass,
    [switch]$NoLegacyLlmSeed,
    [string]$AppDataBackupPath = "",
    [int]$CdpPort = 19322,
    [int]$CdpTimeoutSeconds = 30,
    [int]$ColdStartTimeoutSeconds = 120,
    [int]$ShutdownTimeoutSeconds = 15,
    [int]$HealthTimeoutSeconds = 55,
    [int]$SessionTimeoutSeconds = 90,
    # Dot-source support: define functions/globals without running Invoke-L0.
    [switch]$FunctionsOnly
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ([string]::IsNullOrWhiteSpace($RunId)) {
    $RunId = [guid]::NewGuid().ToString("N").Substring(0, 12)
}
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "Context-OS Client"
}
if ([string]::IsNullOrWhiteSpace($StateHome)) {
    $StateHome = $InstallDir
}

$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$StateHome = [System.IO.Path]::GetFullPath($StateHome)
if ($CdpPort -lt 1 -or $CdpPort -gt 65535) {
    throw "CdpPort must be between 1 and 65535"
}

$global:Signals = @()
$global:Ok = $true
$global:Details = [ordered]@{
    run_id = $RunId
    install_dir = $InstallDir
    state_home = $StateHome
    ports = [ordered]@{
        "5433" = "unknown"
        "6380" = "unknown"
        "18080" = "unknown"
    }
    app_pid = $null
    health_ok = $false
    client_env = [ordered]@{}
    appdata_backup = $AppDataBackupPath
    cdp_port = $CdpPort
    cdp_ready = $false
    window_title = ""
    console_warning = $false
    session_ok = $false
    teardown_ok = $false
}

function Add-Signal {
    param(
        [Parameter(Mandatory = $true)][string]$Level,
        [Parameter(Mandatory = $true)][string]$Code,
        [Parameter(Mandatory = $true)][string]$Reason
    )
    $global:Signals += [ordered]@{
        level = $Level
        id = $Code
        reason = $Reason
    }
    if ($Level -eq "FAIL") {
        $global:Ok = $false
    }
}

function Normalize-Path {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path)) {
        return ""
    }
    try {
        return [System.IO.Path]::GetFullPath($Path.Trim())
    } catch {
        return $Path.Trim()
    }
}

function Test-PathUnder {
    param(
        [string]$Path,
        [string]$Root
    )
    $Path = Normalize-Path $Path
    $Root = Normalize-Path $Root
    if ([string]::IsNullOrWhiteSpace($Path) -or [string]::IsNullOrWhiteSpace($Root)) {
        return $false
    }
    if ([string]::Equals($Path, $Root, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $true
    }
    return $Path.Length -gt $Root.Length -and
        $Path.StartsWith($Root, [System.StringComparison]::OrdinalIgnoreCase) -and
        $Path.Substring($Root.Length).StartsWith("\", [System.StringComparison]::Ordinal)
}

function Test-PathEquals {
    param(
        [string]$Left,
        [string]$Right
    )
    return [string]::Equals(
        (Normalize-Path $Left),
        (Normalize-Path $Right),
        [System.StringComparison]::OrdinalIgnoreCase
    )
}

function Get-PortPids {
    param([Parameter(Mandatory = $true)][int]$Port)
    $pids = @()
    try {
        $conns = @(Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction Stop)
        foreach ($conn in $conns) {
            $localAddress = $conn.LocalAddress.ToString()
            if ($localAddress -in @("127.0.0.1", "0.0.0.0", "::1", "::")) {
                $pids += [int]$conn.OwningProcess
            }
        }
    } catch {
        $netstat = @(& netstat.exe -ano -p tcp 2>$null)
        foreach ($line in $netstat) {
            $parts = @($line -split "\s+" | Where-Object { $_ })
            if ($parts.Count -ge 5 -and $parts[0] -eq "TCP" -and $parts[1] -match ":$([regex]::Escape($Port))$" -and $parts[3] -eq "LISTENING") {
                $pidValue = 0
                if ([int]::TryParse($parts[4], [ref]$pidValue) -and $pidValue -gt 0) {
                    $pids += $pidValue
                }
            }
        }
    }
    return @($pids | Select-Object -Unique)
}

function Get-ProcessPath {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    if ($ProcessId -le 0) {
        return ""
    }
    try {
        $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$ProcessId" -ErrorAction Stop
        if (-not [string]::IsNullOrWhiteSpace($proc.ExecutablePath)) {
            return $proc.ExecutablePath
        }
    } catch {
    }
    try {
        return (Get-Process -Id $ProcessId -ErrorAction Stop).Path
    } catch {
        return ""
    }
}

function Get-ProcessCommandLine {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    if ($ProcessId -le 0) {
        return ""
    }
    try {
        $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$ProcessId" -ErrorAction Stop
        return [string]$proc.CommandLine
    } catch {
        return ""
    }
}

function Test-InstallTree {
    $required = @(
        "Context-OS.exe",
        "avrag-api.exe",
        "avrag-worker.exe",
        "runtime\pgsql\bin\pg_ctl.exe",
        "runtime\redis\redis-server.exe",
        "runtime\pgsql\lib\vector.dll"
    )
    $missing = @()
    foreach ($relative in $required) {
        if (-not (Test-Path (Join-Path $InstallDir $relative))) {
            $missing += $relative
        }
    }
    if ($missing.Count -gt 0) {
        Add-Signal FAIL "S-desktop-tree" "missing install files: $($missing -join ', ')"
        return $false
    }
    return $true
}

function Get-InstallAppProcesses {
    $apps = @(Get-Process -Name "Context-OS" -ErrorAction SilentlyContinue)
    $matched = @()
    foreach ($app in $apps) {
        $path = Get-ProcessPath $app.Id
        if (Test-PathUnder $path $InstallDir) {
            $matched += $app
        }
    }
    return $matched
}

function Test-PortOwner {
    param(
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][string]$Kind
    )
    $pids = @(Get-PortPids $Port)
    if ($pids.Count -eq 0) {
        return "free"
    }
    foreach ($pidValue in $pids) {
        $path = Get-ProcessPath $pidValue
        $owned = (Test-PathUnder $path $InstallDir) -or (Test-PathUnder $path $StateHome)
        if (-not $owned) {
            Add-Signal FAIL "S-desktop-port-owner" "$Kind port $Port owned by foreign pid $pidValue path=$path"
            return "foreign"
        }
    }
    return "owned"
}

function Get-PgDataDir {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $psql = Join-Path $InstallDir "runtime\pgsql\bin\psql.exe"
    if (Test-Path $psql) {
        $previousPassword = $env:PGPASSWORD
        $env:PGPASSWORD = "avrag"
        try {
            $output = @(& $psql -h 127.0.0.1 -p 5433 -U avrag -d postgres -tAc "SHOW data_directory;" 2>$null)
            if ($LASTEXITCODE -eq 0) {
                foreach ($line in $output) {
                    if (-not [string]::IsNullOrWhiteSpace($line)) {
                        return $line.Trim()
                    }
                }
            }
        } catch {
        } finally {
            if ($null -eq $previousPassword) {
                Remove-Item Env:PGPASSWORD -ErrorAction SilentlyContinue
            } else {
                $env:PGPASSWORD = $previousPassword
            }
        }
    }
    $commandLine = Get-ProcessCommandLine $ProcessId
    if ($commandLine -match '-D\s+"([^"]+)"') {
        return $Matches[1]
    }
    if ($commandLine -match '-D\s+([^\s]+)') {
        return $Matches[1]
    }
    return ""
}

function Get-RedisDir {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $redisCli = Join-Path $InstallDir "runtime\redis\redis-cli.exe"
    if (Test-Path $redisCli) {
        $output = @(& $redisCli -h 127.0.0.1 -p 6380 CONFIG GET dir 2>$null)
        if ($LASTEXITCODE -eq 0) {
            $last = @($output | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Last 1)
            if ($last.Count -gt 0) {
                return $last[0].Trim()
            }
        }
    }
    $commandLine = Get-ProcessCommandLine $ProcessId
    if ($commandLine -match '--dir\s+"([^"]+)"') {
        return $Matches[1]
    }
    if ($commandLine -match '--dir\s+([^\s]+)') {
        return $Matches[1]
    }
    return ""
}

function Get-AllowedDataRoots {
    param([string]$Relative)
    $roots = @()
    $roots += (Join-Path $StateHome $Relative)
    if ($UseDefaultTree -and -not (Test-PathEquals $StateHome $InstallDir)) {
        $roots += (Join-Path $InstallDir $Relative)
    }
    return $roots
}

function Test-DataDirOwned {
    param(
        [Parameter(Mandatory = $true)][string]$Actual,
        [Parameter(Mandatory = $true)][string]$Relative,
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][string]$Kind
    )
    $allowed = @(Get-AllowedDataRoots $Relative)
    foreach ($root in $allowed) {
        if (Test-PathEquals $Actual $root) {
            return $true
        }
    }
    Add-Signal FAIL "S-desktop-port-owner" "$Kind port $Port data dir '$Actual' not under $($allowed -join ', ')"
    return $false
}

function Close-AppProcesses {
    $apps = @(Get-InstallAppProcesses)
    if ($apps.Count -eq 0) {
        return $true
    }
    foreach ($app in $apps) {
        try {
            $app.Refresh()
            if ($app.HasExited) {
                continue
            }
            # MainWindowHandle can read 0 transiently while the window is being
            # (re)created — retry briefly before declaring a windowless process.
            $hwndWait = 0
            while (-not $app.HasExited -and $app.MainWindowHandle -eq 0 -and $hwndWait -lt 10) {
                Start-Sleep -Milliseconds 500
                $app.Refresh()
                $hwndWait += 1
            }
            if ($app.HasExited) {
                continue
            }
            if ($app.MainWindowHandle -eq 0) {
                return $false
            }
            $sent = $app.CloseMainWindow()
            if (-not $sent) {
                return $false
            }
        } catch {
            return $false
        }
    }
    $deadline = (Get-Date).AddSeconds($ShutdownTimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $anyRunning = $false
        foreach ($app in $apps) {
            try {
                $app.Refresh()
                if (-not $app.HasExited) {
                    $anyRunning = $true
                }
            } catch {
                $anyRunning = $true
            }
        }
        if (-not $anyRunning) {
            return $true
        }
        Start-Sleep -Milliseconds 250
    }
    return $false
}

function Wait-PortsOpen {
    param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $open = $true
        foreach ($port in @(5433, 6380, 18080)) {
            if (@(Get-PortPids $port).Count -eq 0) {
                $open = $false
                break
            }
        }
        if ($open) {
            return $true
        }
        Start-Sleep -Milliseconds 500
    }
    return $false
}

function Wait-PortsClosed {
    param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $closed = $true
        foreach ($port in @(5433, 6380, 18080)) {
            if (@(Get-PortPids $port).Count -gt 0) {
                $closed = $false
                break
            }
        }
        if ($closed) {
            return $true
        }
        Start-Sleep -Milliseconds 250
    }
    return $false
}

function Test-ApiHealth {
    $deadline = (Get-Date).AddSeconds($HealthTimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $response = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:18080/health" -TimeoutSec 5 -ErrorAction Stop
            if ($response.StatusCode -eq 200) {
                return $true
            }
        } catch {
        }
        Start-Sleep -Milliseconds 500
    }
    return $false
}

function Read-EnvFile {
    param([Parameter(Mandatory = $true)][string]$Path)
    $map = [ordered]@{}
    if (-not (Test-Path $Path)) {
        return $map
    }
    foreach ($line in @(Get-Content $Path -ErrorAction Stop)) {
        $trimmed = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith("#")) {
            continue
        }
        $separator = $trimmed.IndexOf("=")
        if ($separator -gt 0) {
            $key = $trimmed.Substring(0, $separator).Trim()
            $value = $trimmed.Substring($separator + 1).Trim()
            $map[$key] = $value
        }
    }
    return $map
}

function Get-LocalSessionStatus {
    $path = Join-Path $env:APPDATA "com.contextos.desktop\local_session.json"
    $result = [pscustomobject]@{
        valid = $false
        email = ""
        token_present = $false
    }
    if (-not (Test-Path $path)) {
        return $result
    }
    try {
        $data = Get-Content -Raw -Path $path | ConvertFrom-Json
        $result.token_present = -not [string]::IsNullOrWhiteSpace([string]$data.token)
        $result.email = [string]$data.user.email
        $result.valid = $result.token_present -and
            $result.email -eq "local@context-os.client" -and
            (Test-LocalSessionAgainstApi ([string]$data.token))
    } catch {
        return $result
    }
    return $result
}

function Wait-LocalSession {
    param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $status = Get-LocalSessionStatus
        if ($status.valid) {
            return $status
        }
        Start-Sleep -Milliseconds 500
    }
    return $null
}

function Test-LocalSessionAgainstApi {
    param([Parameter(Mandatory = $true)][string]$Token)
    try {
        $headers = @{ Authorization = "Bearer $Token" }
        $response = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:18080/api/auth/me" -Headers $headers -TimeoutSec 5 -ErrorAction Stop
        return $response.StatusCode -eq 200
    } catch {
        return $false
    }
}

function Test-CdpGpoPolicy {
    $policyRoots = @(
        "HKCU:\Software\Policies\Microsoft\Edge\WebView2",
        "HKLM:\Software\Policies\Microsoft\Edge\WebView2"
    )
    foreach ($root in $policyRoots) {
        try {
            $value = (Get-ItemProperty -Path $root -Name AdditionalBrowserArguments -ErrorAction Stop).AdditionalBrowserArguments
            if (-not [string]::IsNullOrWhiteSpace([string]$value) -and [string]$value -notmatch [regex]::Escape($CdpPort.ToString())) {
                Add-Signal FAIL "S-desktop-cdp-gpo" "WebView2 GPO blocks CDP port ${CdpPort}: $value"
                return $false
            }
        } catch {
        }
    }
    return $true
}

function Wait-CdpReady {
    param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $response = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:$CdpPort/json/version" -TimeoutSec 2 -ErrorAction Stop
            if ($response.StatusCode -eq 200) {
                return $true
            }
        } catch {
        }
        Start-Sleep -Milliseconds 500
    }
    return $false
}

function Wait-DesktopWindowTitle {
    param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $apps = @(Get-InstallAppProcesses)
        foreach ($app in $apps) {
            try {
                $app.Refresh()
                if (-not $app.HasExited -and $app.MainWindowHandle -ne 0 -and -not [string]::IsNullOrWhiteSpace($app.MainWindowTitle)) {
                    return [string]$app.MainWindowTitle
                }
            } catch {
            }
        }
        Start-Sleep -Milliseconds 500
    }
    return ""
}

function Test-VisibleConsoleProcesses {
    $visible = @()
    foreach ($name in @("pg_ctl", "curl", "taskkill")) {
        $procs = @(Get-Process -Name $name -ErrorAction SilentlyContinue)
        foreach ($proc in $procs) {
            try {
                $proc.Refresh()
                if (-not $proc.HasExited -and $proc.MainWindowHandle -ne 0) {
                    $visible += "${name}:$($proc.Id)"
                }
            } catch {
            }
        }
    }
    if ($visible.Count -gt 0) {
        Add-Signal WARN "S-desktop-console" "visible console processes: $($visible -join ', ')"
        $global:Details.console_warning = $true
    }
}

function Start-E2eApp {
    $exe = Join-Path $InstallDir "Context-OS.exe"
    Remove-Item Env:CONTEXT_OS_CLIENT_HOME -ErrorAction SilentlyContinue
    $env:CONTEXT_OS_STATE_HOME = $StateHome
    # W3: the cloud login gate blocks the local-stack bootstrap until a session
    # exists. E2E modes (except l3's -NoCloudGateBypass) bypass it — no real
    # cloud account exists here, and without the bypass cold start never opens
    # the stack ports.
    if ($NoCloudGateBypass) {
        Remove-Item Env:CONTEXT_OS_SKIP_CLOUD_GATE -ErrorAction SilentlyContinue
        # l3 only: this box's system proxy (127.0.0.1:20000) is flaky for the
        # cloud host, and reqwest honors system proxy settings by default —
        # pin direct egress for the login path. Production keeps default
        # (system proxy honored); the gate surfaces a clear 云端不可达 error.
        $env:NO_PROXY = "app.contextlm.top"
    } else {
        $env:CONTEXT_OS_SKIP_CLOUD_GATE = "1"
        $global:Details.cloud_gate_bypassed = $true
        if ($env:NO_PROXY -eq "app.contextlm.top") {
            Remove-Item Env:NO_PROXY -ErrorAction SilentlyContinue
        }
    }
    if ($KeepRunning) {
        $webView2Data = Join-Path (Split-Path -Parent $StateHome) "webview2-$RunId"
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$CdpPort --remote-allow-origins=*"
        $env:WEBVIEW2_USER_DATA_FOLDER = $webView2Data
        $global:Details.webview2_data_folder = $webView2Data
        $byokMasterKey = ([guid]::NewGuid().ToString("N") + [guid]::NewGuid().ToString("N"))
        $env:BYOK_MASTER_KEY = $byokMasterKey
        $global:Details.byok_master_key_provisioned = $true
        $env:E2E_ENABLED = "true"
        $global:Details.e2e_enabled = $true
    } else {
        Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
        Remove-Item Env:WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue
        Remove-Item Env:BYOK_MASTER_KEY -ErrorAction SilentlyContinue
        Remove-Item Env:E2E_ENABLED -ErrorAction SilentlyContinue
        Remove-Item Env:MARKITDOWN_BIN -ErrorAction SilentlyContinue
    }
    $process = Start-Process -FilePath $exe -WorkingDirectory $InstallDir -PassThru -WindowStyle Normal
    $process.Refresh()
    return $process
}

function Backup-AppDataFiles {
    if ([string]::IsNullOrWhiteSpace($AppDataBackupPath)) {
        return
    }
    & "$PSScriptRoot\backup-appdata.ps1" -Action Backup -BackupPath $AppDataBackupPath
    if ($LASTEXITCODE -ne 0) {
        throw "AppData backup failed"
    }
}

function Seed-LegacyLlm {
    if (-not $KeepRunning) {
        return
    }
    & "$PSScriptRoot\seed-legacy-llm.ps1" -Action Seed
    if ($LASTEXITCODE -ne 0) {
        throw "Legacy LLM config seed failed"
    }
}

function Restore-AppDataFiles {
    if ([string]::IsNullOrWhiteSpace($AppDataBackupPath)) {
        return
    }
    & "$PSScriptRoot\backup-appdata.ps1" -Action Restore -BackupPath $AppDataBackupPath
    if ($LASTEXITCODE -ne 0) {
        throw "AppData restore failed"
    }
}

function Write-Result {
    $outDir = Join-Path $env:TEMP "cos-e2e-$RunId"
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    $suffix = if ($KeepRunning) { "-keep" } elseif ($TeardownOnly) { "-teardown" } else { "" }
    $jsonPath = Join-Path $outDir "l0$suffix.json"
    $signalsPath = Join-Path $outDir "signals$suffix.txt"
    $result = [ordered]@{
        ok = $global:Ok
        details = $global:Details
        signals = @($global:Signals)
    }
    $result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 -Path $jsonPath
    $global:Signals | ForEach-Object {
        "$($_.level) $($_.id) $($_.reason)"
    } | Set-Content -Encoding UTF8 -Path $signalsPath
    Write-Host "E2E_L0_OUTPUT=$jsonPath"
}

function Invoke-L0 {
    $global:Details.install_dir = $InstallDir
    $global:Details.state_home = $StateHome

    if (-not (Test-InstallTree)) {
        Write-Result
        return
    }

    foreach ($port in @(5433, 6380, 18080)) {
        $kind = switch ($port) {
            5433 { "postgres" }
            6380 { "redis" }
            18080 { "api" }
            default { "unknown" }
        }
        $global:Details.ports["$port"] = Test-PortOwner $port $kind
    }

    $ownedPorts = @()
    foreach ($port in @(5433, 6380, 18080)) {
        if ($global:Details.ports["$port"] -eq "owned") {
            $ownedPorts += $port
        }
    }

    foreach ($port in $ownedPorts) {
        $pids = @(Get-PortPids $port)
        if ($pids.Count -eq 0) {
            continue
        }
        if ($port -eq 5433) {
            $dataDir = Get-PgDataDir $pids[0]
            if (-not (Test-DataDirOwned $dataDir "data\pg-native" $port "postgres")) {
                Write-Result
                return
            }
        }
        if ($port -eq 6380) {
            $dataDir = Get-RedisDir $pids[0]
            if (-not (Test-DataDirOwned $dataDir "data\redis-native" $port "redis")) {
                Write-Result
                return
            }
        }
    }

    if ($AuditOnly) {
        Write-Result
        return
    }

    if ($TeardownOnly) {
        $apps = @(Get-InstallAppProcesses)
        if ($ownedPorts.Count -gt 0 -or $apps.Count -gt 0) {
            $closed = Close-AppProcesses
            if (-not $closed) {
                Add-Signal FAIL "S-desktop-no-app-window" "teardown found same-tree Context-OS.exe without a closeable main window"
                Write-Result
                return
            }
            if (-not (Wait-PortsClosed $ShutdownTimeoutSeconds)) {
                Add-Signal FAIL "S-desktop-shutdown-timeout" "ports did not release during teardown"
                Write-Result
                return
            }
        }
        Restore-AppDataFiles
        $global:Details.teardown_ok = $true
        Write-Result
        return
    }

    $apps = @(Get-InstallAppProcesses)
    if ($ownedPorts.Count -gt 0 -or $apps.Count -gt 0) {
        $closed = Close-AppProcesses
        if (-not $closed) {
            if ($ownedPorts.Count -gt 0) {
                Add-Signal FAIL "S-desktop-no-app-window" "same-tree ports owned but Context-OS.exe main window cannot be closed"
            } else {
                Add-Signal WARN "S-desktop-no-app-window" "same-tree Context-OS.exe has no closeable main window; skipping launch"
            }
            Write-Result
            return
        }
        if (-not (Wait-PortsClosed $ShutdownTimeoutSeconds)) {
            Add-Signal FAIL "S-desktop-shutdown-timeout" "ports did not release after closing Context-OS.exe"
            Write-Result
            return
        }
    }

    if ($KeepRunning -and -not (Test-CdpGpoPolicy)) {
        Write-Result
        return
    }

    Backup-AppDataFiles
    if (-not $NoLegacyLlmSeed) {
        Seed-LegacyLlm
    }
    $process = Start-E2eApp
    $global:Details.app_pid = $process.Id
    if ($NoCloudGateBypass) {
        # l3: the real W3 login gate blocks the local-stack bootstrap until the
        # spec signs in — stack ports/health/client.env/session are post-login
        # concerns. Keep asserts only what the gate allows: window + CDP.
        $windowTitle = Wait-DesktopWindowTitle 30
        $global:Details.window_title = $windowTitle
        if ([string]::IsNullOrWhiteSpace($windowTitle) -or $windowTitle -notmatch "Context-OS Client") {
            Add-Signal FAIL "S-desktop-cold" "expected Context-OS Client window title; got '$windowTitle'"
        }
        $cdpReady = Wait-CdpReady $CdpTimeoutSeconds
        $global:Details.cdp_ready = $cdpReady
        if (-not $cdpReady) {
            Add-Signal FAIL "S-desktop-cdp" "WebView2 CDP did not open on port $CdpPort"
        }
        Write-Result
        return
    }
    if (-not (Wait-PortsOpen $ColdStartTimeoutSeconds)) {
        Add-Signal FAIL "S-desktop-port" "cold start ports did not open within ${ColdStartTimeoutSeconds}s"
        Write-Result
        return
    }

    foreach ($port in @(5433, 6380, 18080)) {
        $kind = switch ($port) {
            5433 { "postgres" }
            6380 { "redis" }
            18080 { "api" }
            default { "unknown" }
        }
        $owner = Test-PortOwner $port $kind
        if ($owner -ne "owned") {
            Add-Signal FAIL "S-desktop-port-owner" "$kind port $port is not owned by the E2E tree after cold start"
            Write-Result
            return
        }
    }
    $pgPids = @(Get-PortPids 5433)
    $redisPids = @(Get-PortPids 6380)
    if ($pgPids.Count -eq 0 -or $redisPids.Count -eq 0) {
        Add-Signal FAIL "S-desktop-port" "PG/Redis ports disappeared during ownership recheck"
        Write-Result
        return
    }
    $pgData = Get-PgDataDir $pgPids[0]
    if (-not (Test-DataDirOwned $pgData "data\pg-native" 5433 "postgres")) {
        Write-Result
        return
    }
    $redisData = Get-RedisDir $redisPids[0]
    if (-not (Test-DataDirOwned $redisData "data\redis-native" 6380 "redis")) {
        Write-Result
        return
    }

    $healthOk = Test-ApiHealth
    $global:Details.health_ok = $healthOk
    if (-not $healthOk) {
        Add-Signal FAIL "S-desktop-port" "local API health did not return 200 within ${HealthTimeoutSeconds}s"
        Write-Result
        return
    }

    $windowTitle = Wait-DesktopWindowTitle 30
    $global:Details.window_title = $windowTitle
    if ([string]::IsNullOrWhiteSpace($windowTitle) -or $windowTitle -notmatch "Context-OS Client") {
        Add-Signal FAIL "S-desktop-cold" "expected Context-OS Client window title; got '$windowTitle'"
    }
    Test-VisibleConsoleProcesses

    if ($KeepRunning) {
        $cdpReady = Wait-CdpReady $CdpTimeoutSeconds
        $global:Details.cdp_ready = $cdpReady
        if (-not $cdpReady) {
            Add-Signal FAIL "S-desktop-cdp" "WebView2 CDP did not open on port $CdpPort"
            Write-Result
            return
        }
    }

    $envPath = Join-Path $StateHome "client.env"
    if (-not (Test-Path $envPath)) {
        Add-Signal FAIL "S-desktop-env" "missing client.env at $envPath"
    } else {
        $envMap = Read-EnvFile $envPath
        $global:Details.client_env = $envMap
        if (-not $envMap.Contains("RETRIEVAL_BACKEND") -or $envMap["RETRIEVAL_BACKEND"] -ne "pgvector") {
            Add-Signal FAIL "S-desktop-env" "RETRIEVAL_BACKEND must be pgvector"
        }
        if (-not $envMap.Contains("AVRAG_ENABLE_RAG") -or $envMap["AVRAG_ENABLE_RAG"] -ne "true") {
            Add-Signal FAIL "S-desktop-env" "AVRAG_ENABLE_RAG must be true (PR-4 Layer B: RAG enabled from SiliconFlow secret)"
        }
    }

    $sessionStatus = Wait-LocalSession $SessionTimeoutSeconds
    if ($null -eq $sessionStatus -or -not $sessionStatus.valid) {
        Add-Signal FAIL "S-desktop-session" "local@context-os.client session was not available after product bootstrap"
    } else {
        $global:Details.session_ok = $true
    }

    if ($KeepRunning) {
        Write-Result
        return
    }

    $closed = Close-AppProcesses
    if (-not $closed) {
        Add-Signal FAIL "S-desktop-no-app-window" "E2E Context-OS.exe main window cannot be closed"
        Write-Result
        return
    }
    if (-not (Wait-PortsClosed $ShutdownTimeoutSeconds)) {
        Add-Signal FAIL "S-desktop-shutdown-timeout" "ports did not release within ${ShutdownTimeoutSeconds}s after close"
        Write-Result
        return
    }
    Restore-AppDataFiles
    $global:Details.teardown_ok = $true
    Write-Result
}

if ($FunctionsOnly) {
    return
}

try {
    Invoke-L0
} catch {
    Add-Signal FAIL "S-desktop-script" "unhandled L0 error: $($_.Exception.Message)"
    Write-Result
}

if ($global:Ok) {
    exit 0
}
exit 1
