param(
    [string]$Workspace = 'C:\dev\context-osv6',
    [string]$BuildDir = 'C:\dev\gpui-backend-target\x86_64-pc-windows-gnu\debug',
    [string]$PortableRuntime = (Join-Path $env:LOCALAPPDATA 'Context-OS Client\runtime'),
    [int]$ApiPort = 18190,
    [int]$PgPort = 15439,
    [int]$RedisPort = 16389,
    [switch]$DataPlaneOnly
)
$ErrorActionPreference = 'Stop'
$source = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$project = Join-Path $Workspace 'desktop_gpui'
$root = Join-Path $project ('target\acceptance\managed\gpui-managed-' + [Guid]::NewGuid().ToString('N'))
$ports = @($ApiPort, $PgPort, $RedisPort)
if (($ports | Select-Object -Unique).Count -ne 3 -or ($ports | Where-Object { $_ -lt 1024 -or $_ -gt 65535 })) { throw 'Invalid or duplicate test ports.' }
foreach ($port in $ports) {
    if (Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue) { throw "Port $port is occupied; nothing started." }
}
$pgBin = Join-Path $PortableRuntime 'pgsql\bin'
$redisBin = Join-Path $PortableRuntime 'redis\redis-server.exe'
$binaries = @('avrag-api.exe', 'avrag-worker.exe', 'avrag-migrate.exe')
if ($DataPlaneOnly) { $binaries = @() }
foreach ($file in @((Join-Path $pgBin 'pg_ctl.exe'), (Join-Path $pgBin 'initdb.exe'), $redisBin) + @($binaries | ForEach-Object { Join-Path $BuildDir $_ })) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Required binary missing: $file" }
}
$processes = @(Get-CimInstance Win32_Process)
$postgresIds = @($processes | Where-Object Name -eq 'postgres.exe' | ForEach-Object ProcessId)
# PostgreSQL session children may end naturally. Preserve the postmaster/service
# identities, rather than treating normal connection-pool turnover as a restart.
$existing = @($processes | Where-Object {
    $_.Name -in @('avrag-api.exe', 'avrag-worker.exe', 'postgres.exe', 'redis-server.exe', 'desktop-gpui.exe') -and
    -not ($_.Name -eq 'postgres.exe' -and $_.ParentProcessId -in $postgresIds)
} | Select-Object ProcessId, ExecutablePath, CreationDate)
New-Item -ItemType Directory -Path (Join-Path $root 'bin') -Force | Out-Null
Set-Content -LiteralPath (Join-Path $root 'acceptance.marker') -Value 'Isolated GPUI managed lifecycle acceptance.'
# Stop dotenv's upward search; all runtime values come from this test's client.env.
Set-Content -LiteralPath (Join-Path $root 'bin\.env') -Value '# Isolated process fixture; environment is supplied by desktop-core.'
foreach ($name in $binaries) { Copy-Item -LiteralPath (Join-Path $BuildDir $name) -Destination (Join-Path $root 'bin') }
foreach ($name in @('libgcc_s_seh-1.dll', 'libstdc++-6.dll', 'libwinpthread-1.dll') | Where-Object { -not $DataPlaneOnly }) {
    $dll = Join-Path $BuildDir $name
    if (-not (Test-Path -LiteralPath $dll)) { $dll = Join-Path 'C:\dev\gpui-acceptance-20260909\backend-current' $name }
    Copy-Item -LiteralPath $dll -Destination (Join-Path $root 'bin')
}
foreach ($part in @('prompts', 'modes')) {
    Copy-Item -LiteralPath (Join-Path $source "avrag-rs\$part") -Destination (Join-Path $root 'bin') -Recurse
}
Copy-Item -LiteralPath (Join-Path $source 'avrag-rs\migrations') -Destination $root -Recurse
$hashes = @($binaries | ForEach-Object { Get-FileHash -LiteralPath (Join-Path $root "bin\$_") -Algorithm SHA256 | Select-Object Path, Hash })
$hashes | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'binaries.json')
$sourceHashes = [ordered]@{}
foreach ($file in @('desktop_gpui/Cargo.lock', 'desktop_gpui/tests/managed_lifecycle.rs', 'desktop_gpui/src/runtime.rs', 'desktop_gpui/src/services.rs', 'desktop/core/src/local_product.rs', 'desktop/core/src/local_stack.rs', 'desktop/core/src/native_stack.rs', 'desktop/core/src/process_deadline.rs', 'desktop/core/src/runtime_lease.rs', 'desktop/core/src/runtime_ports.rs', 'desktop/core/src/win_cmd.rs')) {
    $sourceHashes[$file] = (Get-FileHash -LiteralPath (Join-Path $Workspace $file) -Algorithm SHA256).Hash
}
$saved = @{}
Get-ChildItem Env: | ForEach-Object { $saved[$_.Name] = $_.Value }
$started = Get-Date
$failure = $null
$exitCode = -1
Push-Location $project
try {
    # No inherited provider, cloud or database configuration in the process fixture.
    Get-ChildItem Env: | Where-Object { $_.Name -match '^(CLIENT_|AVRAG_|CONTEXT_OS_|AGENT_|EMBEDDING_|INGESTION_|RERANK_|S3_|AWS_|COS_|PG_|REDIS_|DATABASE_URL|MIGRATION_DATABASE_URL|JWT_SECRET|BYOK_MASTER_KEY)' -or $_.Name -match '_API_KEY$' } | ForEach-Object { Remove-Item -LiteralPath "Env:$($_.Name)" }
    $env:CARGO_BUILD_JOBS = '2'
    $env:CARGO_TARGET_DIR = Join-Path $project 'target'
    $env:GPUI_ACCEPTANCE_MANAGED = '1'
    $env:CONTEXT_OS_CLIENT_HOME = $root
    $env:CONTEXT_OS_RUNTIME = $root
    $env:PG_BIN_DIR = $pgBin
    $env:REDIS_SERVER_BIN = $redisBin
    $env:CLIENT_API_PORT = $ApiPort.ToString()
    $env:CLIENT_PG_PORT = $PgPort.ToString()
    $env:CLIENT_REDIS_PORT = $RedisPort.ToString()
    $env:AVRAG_WORKER_HEALTH_PORT = '0'
    $env:AVRAG_WORKER_HEALTH_PORT_FILE = Join-Path $root 'worker-health.port'
    $env:RUST_LOG = 'info'
    $testName = if ($DataPlaneOnly) { 'native_data_plane_cold_start_and_shutdown' } else { 'managed_cold_start_failure_recovery_and_shutdown' }
    & cargo test --locked --test managed_lifecycle -- --ignored --exact $testName --nocapture --test-threads=1 *> (Join-Path $root 'tests.log')
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) { throw "Managed lifecycle test failed; see $root\tests.log" }
    $log = Get-Content -Raw -LiteralPath (Join-Path $root 'tests.log')
    if ($log -notmatch 'test result: ok\. 1 passed; 0 failed; 0 ignored;') { throw 'No complete managed test result.' }
    $steps = @(Get-Content -Raw -LiteralPath (Join-Path $root 'steps.json') | ConvertFrom-Json)
    $expectedSteps = if ($DataPlaneOnly) { 4 } else { 8 }
    if ($steps.Count -ne $expectedSteps) { throw 'Incomplete managed lifecycle steps.' }
} catch { $failure = $_.Exception.Message }
finally {
    Pop-Location
    $escapedRoot = [regex]::Escape($root)
    $remaining = @(Get-CimInstance Win32_Process | Where-Object {
        ($_.ExecutablePath -and $_.ExecutablePath.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) -or
        ($_.Name -in @('postgres.exe', 'redis-server.exe') -and $_.CommandLine -match $escapedRoot)
    } | Select-Object ProcessId, Name)
    if ($remaining.Count) { $failure = "Isolated processes remain after cleanup: $($remaining | ConvertTo-Json -Compress). $failure" }
    foreach ($process in $existing) {
        $current = Get-CimInstance Win32_Process -Filter "ProcessId = $($process.ProcessId)"
        if (-not $current -or $current.ExecutablePath -ne $process.ExecutablePath -or $current.CreationDate -ne $process.CreationDate) {
            $failure = "Pre-existing process identity changed: $($process.ProcessId). $failure"
        }
    }
    $result = [ordered]@{
        passed = ($null -eq $failure); exitCode = $exitCode; error = $failure
        seconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 2)
        root = $root; ports = $ports; binaries = $hashes; sourceHashes = $sourceHashes
        preservedExistingProcesses = $existing.Count; remainingProcesses = $remaining
        scope = $(if ($DataPlaneOnly) { 'Real isolated PG/Redis and desktop-core; product processes not exercised.' } else { 'Real isolated PG/Redis/migrate/API/worker and GPUI Host. No desktop automation or model calls.' })
    }
    $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $root 'result.json')
    Get-ChildItem Env: | Where-Object { -not $saved.ContainsKey($_.Name) } | ForEach-Object { Remove-Item -LiteralPath "Env:$($_.Name)" }
    foreach ($key in $saved.Keys) { Set-Item -LiteralPath "Env:$key" -Value $saved[$key] }
}
Get-Content -Raw -LiteralPath (Join-Path $root 'result.json')
if ($failure) { throw $failure }
