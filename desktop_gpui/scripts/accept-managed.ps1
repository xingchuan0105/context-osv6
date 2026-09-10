param(
    [string]$Workspace = 'C:\dev\context-osv6',
    [string]$BuildDir = 'C:\dev\gpui-backend-target\x86_64-pc-windows-gnu\debug',
    [string]$PortableRuntime = (Join-Path $env:LOCALAPPDATA 'Context-OS Client\runtime'),
    [int]$ApiPort = 18190,
    [int]$PgPort = 15439,
    [int]$RedisPort = 16389,
    [switch]$DataPlaneOnly,
    [switch]$OfficeRag
)
$ErrorActionPreference = 'Stop'
if ($DataPlaneOnly -and $OfficeRag) { throw 'Choose data-plane or Office/RAG acceptance.' }
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
    if (-not (Test-Path -LiteralPath $dll)) { throw "Matching build runtime missing: $dll. Stage with scripts/stage-mingw-runtime.sh; installed product DLLs may use a different thread model." }
    Copy-Item -LiteralPath $dll -Destination (Join-Path $root 'bin')
}
foreach ($part in @('prompts', 'modes')) {
    Copy-Item -LiteralPath (Join-Path $source "avrag-rs\$part") -Destination (Join-Path $root 'bin') -Recurse
}
Copy-Item -LiteralPath (Join-Path $source 'avrag-rs\migrations') -Destination $root -Recurse
$hashes = @(Get-ChildItem -LiteralPath (Join-Path $root 'bin') -File | Where-Object Extension -in @('.exe', '.dll') | Get-FileHash -Algorithm SHA256 | Select-Object Path, Hash)
$hashes | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'binaries.json')
if ($OfficeRag) {
    # Same package staging used by the Windows installer; isolated copy only.
    $installedPython = Join-Path (Split-Path $PortableRuntime -Parent) 'python'
    if (-not (Test-Path -LiteralPath (Join-Path $installedPython 'python.exe'))) { throw 'Embedded Python missing.' }
    Copy-Item -LiteralPath $installedPython -Destination (Join-Path $root 'bin\python') -Recurse
    New-Item -ItemType Directory -Path (Join-Path $root 'parsers') -Force | Out-Null
    foreach ($file in @('markitdown-lite.cmd', 'markitdown_lite.py')) {
        Copy-Item -LiteralPath (Join-Path $source "desktop\runtime\parsers\$file") -Destination (Join-Path $root 'parsers')
    }
    & (Join-Path $root 'bin\python\python.exe') (Join-Path $source 'scripts\stage-desktop-office.py') --python-dir (Join-Path $root 'bin\python') --parsers-dir (Join-Path $root 'parsers') *> (Join-Path $root 'office-stage.log')
    if ($LASTEXITCODE -ne 0) { throw "Office staging failed; see $root\office-stage.log" }
    New-Item -ItemType Directory -Path (Join-Path $root 'fixtures') -Force | Out-Null
    foreach ($file in @('frontend_rust\tests\fixtures\live-office\inventory.xlsx', 'frontend_rust\tests\fixtures\live-office\delivery.pptx', 'avrag-rs\crates\app\tests\product_e2e\fixtures\phase0-mini.docx')) {
        Copy-Item -LiteralPath (Join-Path $source $file) -Destination (Join-Path $root 'fixtures')
    }
}
$sourceHashes = [ordered]@{}
foreach ($file in @('desktop_gpui/Cargo.lock', 'desktop_gpui/tests/managed_lifecycle.rs', 'desktop_gpui/src/runtime.rs', 'desktop_gpui/src/services.rs', 'desktop/core/src/local_product.rs', 'desktop/core/src/local_stack.rs', 'desktop/core/src/native_stack.rs', 'desktop/core/src/process_deadline.rs', 'desktop/core/src/runtime_lease.rs', 'desktop/core/src/runtime_ports.rs', 'desktop/core/src/win_cmd.rs')) {
    $sourceHashes[$file] = (Get-FileHash -LiteralPath (Join-Path $Workspace $file) -Algorithm SHA256).Hash
}
if ($OfficeRag) {
    $sourceHashes['desktop_gpui/tests/live_rag.rs'] = (Get-FileHash -LiteralPath (Join-Path $Workspace 'desktop_gpui\tests\live_rag.rs')).Hash
    Get-ChildItem -LiteralPath (Join-Path $root 'fixtures'), (Join-Path $root 'bin\prompts\eval\gpui-office') -File | ForEach-Object {
        $sourceHashes[[IO.Path]::GetRelativePath($root, $_.FullName)] = (Get-FileHash -LiteralPath $_.FullName).Hash
    }
}
$saved = @{}
Get-ChildItem Env: | ForEach-Object { $saved[$_.Name] = $_.Value }
$started = Get-Date
$failure = $null
$exitCode = -1
Push-Location $project
try {
    # No inherited provider, cloud or database configuration in the process fixture.
    Get-ChildItem Env: | Where-Object { $_.Name -match '^(CLIENT_|AVRAG_|CONTEXT_OS_|AGENT_|EMBEDDING_|INGESTION_|RERANK_|TRIPLET_|QUICK_CHAT_|PLATFORM_|S3_|AWS_|COS_|PG_|REDIS_|DATABASE_URL|MIGRATION_DATABASE_URL|JWT_SECRET|BYOK_MASTER_KEY)' -or $_.Name -match '_API_KEY$' } | ForEach-Object { Remove-Item -LiteralPath "Env:$($_.Name)" }
    if ($OfficeRag) {
        # Only model/price settings cross into this environment. Database,
        # account, upload, storage and cloud relay state stay newly generated.
        $providerConfig = @{}
        foreach ($line in Get-Content -LiteralPath (Join-Path $source 'avrag-rs\.env')) {
            if ($line -match '^((?:AGENT_LLM_|INGESTION_LLM_|EMBEDDING_|RERANK_|TRIPLET_LLM_|QUICK_CHAT_LLM_)[A-Z_]+|PLATFORM_OFFICIAL_RATES_JSON|INGESTION_TRIPLET_ENABLED|INGESTION_VLM_SUMMARY_ENABLED|INGESTION_VLM_TRIPLET_ENABLED)=(.*)$') {
                $key = $Matches[1]
                $value = $Matches[2].Trim().Trim('"').Trim("'")
                $providerConfig[$key] = $value
                if ($value) { Set-Item -LiteralPath "Env:$key" -Value $value }
            }
        }
        foreach ($key in @('AGENT_LLM_API_KEY', 'INGESTION_LLM_API_KEY', 'EMBEDDING_API_KEY', 'PLATFORM_OFFICIAL_RATES_JSON')) {
            if (-not $providerConfig[$key]) { throw "Configured $key is required; no model run started." }
        }
        [ordered]@{agentModel=$providerConfig['AGENT_LLM_MODEL'];ingestionModel=$providerConfig['INGESTION_LLM_MODEL'];embeddingModel=$providerConfig['EMBEDDING_MODEL'];rerankModel=$providerConfig['RERANK_MODEL'];credentials='Reused configured environment values; omitted from report.'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'models.json')
        $env:GPUI_ACCEPTANCE_OFFICE_RAG = '1'
        $env:AVRAG_PLATFORM_KEYS_RELAY = 'false'
    }
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
    $testName = if ($OfficeRag) { 'office_ingestion_and_scoped_rag' } elseif ($DataPlaneOnly) { 'native_data_plane_cold_start_and_shutdown' } else { 'managed_cold_start_failure_recovery_and_shutdown' }
    $testTarget = if ($OfficeRag) { 'live_rag' } else { 'managed_lifecycle' }
    & cargo test --locked --test $testTarget -- --ignored --exact $testName --nocapture --test-threads=1 *> (Join-Path $root 'tests.log')
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) { throw "Managed lifecycle test failed; see $root\tests.log" }
    $log = Get-Content -Raw -LiteralPath (Join-Path $root 'tests.log')
    if ($log -notmatch 'test result: ok\. 1 passed; 0 failed; 0 ignored;') { throw 'No complete managed test result.' }
    $steps = @(Get-Content -Raw -LiteralPath (Join-Path $root 'steps.json') | ConvertFrom-Json)
    if ($OfficeRag) {
        $journey = Get-Content -Raw -LiteralPath (Join-Path $root 'journey.json') | ConvertFrom-Json
        if (-not $journey.ok -or $journey.documents.Count -ne 3 -or $journey.sessions.Count -ne 3) { throw 'Incomplete Office/RAG journey.' }
    } else {
        $expectedSteps = if ($DataPlaneOnly) { 4 } else { 8 }
        if ($steps.Count -ne $expectedSteps) { throw 'Incomplete managed lifecycle steps.' }
    }
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
        scope = $(if ($OfficeRag) { 'Real isolated native stack, GPUI Host, three Office files, configured embedding/LLM providers, scoped RAG and metering. No desktop automation or existing-service restart.' } elseif ($DataPlaneOnly) { 'Real isolated PG/Redis and desktop-core; product processes not exercised.' } else { 'Real isolated PG/Redis/migrate/API/worker and GPUI Host. No desktop automation or model calls.' })
    }
    $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $root 'result.json')
    Get-ChildItem Env: | Where-Object { -not $saved.ContainsKey($_.Name) } | ForEach-Object { Remove-Item -LiteralPath "Env:$($_.Name)" }
    foreach ($key in $saved.Keys) { Set-Item -LiteralPath "Env:$key" -Value $saved[$key] }
}
Get-Content -Raw -LiteralPath (Join-Path $root 'result.json')
if ($failure) { throw $failure }
