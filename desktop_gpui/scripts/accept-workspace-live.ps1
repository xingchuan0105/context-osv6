param(
    [string]$Workspace = 'C:\dev\context-osv6',
    [string]$StateDir = 'C:\dev\gpui-acceptance-20260909'
)
$ErrorActionPreference = 'Stop'
$stateRoot = (Get-Item -LiteralPath $StateDir).FullName
$project = Join-Path ([IO.Path]::GetFullPath($Workspace)) 'desktop_gpui'
$sessionDir = Join-Path $stateRoot 'session-current'
$tracked = Get-Content -LiteralPath (Join-Path $stateRoot 'processes.jsonl') | ForEach-Object { $_ | ConvertFrom-Json }
$api = $tracked | Where-Object name -eq 'api' | Select-Object -Last 1
$process = Get-CimInstance Win32_Process -Filter "ProcessId = $($api.id)"
$listener = Get-NetTCPConnection -State Listen -LocalPort 18082 -ErrorAction Stop
if (-not $process -or $listener.OwningProcess -ne $api.id -or $process.ExecutablePath -ne $api.path -or -not $process.ExecutablePath.StartsWith($stateRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The API is not the recorded isolated acceptance service. Nothing changed.'
}
if (-not (Test-Path -LiteralPath (Join-Path $sessionDir 'local_session.json'))) { throw 'Existing isolated session missing.' }
$health = Invoke-RestMethod 'http://127.0.0.1:18082/health' -TimeoutSec 5
if ($health.status -ne 'ok') { throw 'Existing isolated API is unhealthy.' }
$existing = @(Get-CimInstance Win32_Process | Where-Object Name -in @('desktop-gpui.exe','avrag-api.exe','avrag-worker.exe','redis-server.exe') | Select-Object ProcessId,ExecutablePath,CreationDate)
$runDir = Join-Path $project ('target\acceptance\workspace-live\gpui-workspace-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $runDir -Force | Out-Null
$settings = @{
    CARGO_BUILD_JOBS = '2'; CARGO_TARGET_DIR = (Join-Path $project 'target')
    CLIENT_API_BASE_URL = 'http://127.0.0.1:18082'; AVRAG_PUBLIC_BASE_URL = 'http://127.0.0.1:18082'
    CONTEXT_OS_CLIENT_HOME = $stateRoot; CONTEXT_OS_DESKTOP_DATA_DIR = $sessionDir
    GPUI_ACCEPTANCE_REPORT_DIR = $runDir; GPUI_ACCEPTANCE_LIVE_WORKSPACE = '1'
}
$previous = @{}
foreach ($key in $settings.Keys) {
    $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
    [Environment]::SetEnvironmentVariable($key, $settings[$key], 'Process')
}
$started = Get-Date
Push-Location $project
try {
    & cargo test --locked --test live_workspace -- --ignored --test-threads=1 *> (Join-Path $runDir 'tests.log')
    $exitCode = $LASTEXITCODE
    $unchanged = $true
    foreach ($before in $existing) {
        $after = Get-CimInstance Win32_Process -Filter "ProcessId = $($before.ProcessId)"
        if (-not $after -or $after.ExecutablePath -ne $before.ExecutablePath -or $after.CreationDate -ne $before.CreationDate) { $unchanged = $false }
    }
    $journeyPath = Join-Path $runDir 'journey.json'
    $journey = if (Test-Path -LiteralPath $journeyPath) { Get-Content -Raw -LiteralPath $journeyPath | ConvertFrom-Json } else { $null }
    $hashes = [ordered]@{}
    foreach ($file in @('Cargo.lock','src/workspace.rs','src/runtime.rs','tests/live_workspace.rs','../desktop/core/src/api_proxy.rs')) {
        $hashes[$file] = (Get-FileHash -LiteralPath (Join-Path $project $file)).Hash
    }
    $ok = $exitCode -eq 0 -and $journey -and $journey.ok -and $unchanged
    [ordered]@{
        ok = [bool]$ok; exitCode = $exitCode; seconds = [math]::Round(((Get-Date)-$started).TotalSeconds,2)
        existingProcessesUnchanged = $unchanged; runDirectory = $runDir; sourceHashes = $hashes
        backend = @{path=$process.ExecutablePath; pid=$process.ProcessId; started=$process.CreationDate; sha256=(Get-FileHash -LiteralPath $process.ExecutablePath).Hash}
        command = 'cargo test --locked --test live_workspace -- --ignored --test-threads=1'
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $runDir 'result.json') -Encoding utf8
    Get-Content -LiteralPath (Join-Path $runDir 'result.json')
    if (-not $ok) { throw "Live workspace acceptance failed; see $runDir" }
} finally {
    Pop-Location
    foreach ($key in $settings.Keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process') }
}
