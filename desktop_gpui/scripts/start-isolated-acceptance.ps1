param(
    [string]$Workspace = 'C:\dev\context-osv6',
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Context-OS Client'),
    [string]$StateDir = 'C:\dev\gpui-acceptance-20260909',
    [switch]$ResumeMigration
)
$ErrorActionPreference = 'Stop'
$stateRoot = [IO.Path]::GetFullPath($StateDir)
if ((Test-Path -LiteralPath $stateRoot) -and -not $ResumeMigration) { throw 'Acceptance directory already exists; refusing to overwrite or adopt it.' }
foreach ($port in $(if ($ResumeMigration) { @(18082) } else { @(15433,16380,18082) })) {
    if (Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue) { throw "Port $port is occupied; no existing service will be stopped." }
}
$pgBin = Join-Path $InstallDir 'runtime\pgsql\bin'
$exe = Join-Path $Workspace 'desktop_gpui\target\debug\desktop-gpui.exe'
$sourceRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
foreach ($file in @($exe, "$pgBin\initdb.exe", "$pgBin\psql.exe", "$InstallDir\avrag-api.exe", "$InstallDir\avrag-migrate.exe", "$InstallDir\runtime\redis\redis-server.exe")) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing prerequisite: $file" }
}
if ($ResumeMigration) {
    $tracked = Get-Content "$stateRoot\processes.jsonl" | ConvertFrom-Json
    foreach ($name in @('postgres','redis')) {
        $entry = $tracked | Where-Object name -eq $name | Select-Object -Last 1
        $running = Get-CimInstance Win32_Process -Filter "ProcessId = $($entry.id)"
        if ($running.ExecutablePath -ne $entry.path -or -not $running.CommandLine.Contains($stateRoot)) { throw 'Isolated service ownership could not be verified.' }
    }
    if (Get-ChildItem "$stateRoot\session" -File) { throw 'ResumeMigration is only for a run that has not logged in.' }
} else {
    New-Item -ItemType Directory -Path $stateRoot | Out-Null
    foreach ($dir in @('logs','objects','redis','session')) { New-Item -ItemType Directory -Path (Join-Path $stateRoot $dir) | Out-Null }
}
function Run-Psql([string]$Database, [string]$Sql) {
    & "$pgBin\psql.exe" -h 127.0.0.1 -p 15433 -U avrag_cluster_admin -d $Database -v ON_ERROR_STOP=1 -c $Sql *> "$stateRoot\logs\sql.log"
    if ($LASTEXITCODE -ne 0) { throw "SQL failed; see $stateRoot\logs\sql.log" }
}
function Start-Tracked([string]$Name, [string]$File, [string[]]$Arguments) {
    $parameters = @{FilePath=$File; WorkingDirectory=$InstallDir; WindowStyle='Hidden'; PassThru=$true; RedirectStandardOutput="$stateRoot\logs\$Name.out.log"; RedirectStandardError="$stateRoot\logs\$Name.err.log"}
    if ($Arguments.Count) { $parameters.ArgumentList=$Arguments }
    $child = Start-Process @parameters
    Add-Content -LiteralPath "$stateRoot\processes.jsonl" -Value (@{name=$Name;id=$child.Id;path=$File} | ConvertTo-Json -Compress)
    return $child
}
function Wait-Port([int]$Port) {
    for ($i=0; $i -lt 60; $i++) {
        if (Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue) { return }
        Start-Sleep -Milliseconds 500
    }
    throw "Timeout on port $Port; logs in $stateRoot\logs"
}
if (-not $ResumeMigration) {
& "$pgBin\initdb.exe" -D "$stateRoot\pg" -U avrag_cluster_admin --auth=trust --encoding=UTF8 --locale=C *> "$stateRoot\logs\initdb.log"
if ($LASTEXITCODE -ne 0) { throw 'initdb failed' }
$null = Start-Tracked 'postgres' "$pgBin\postgres.exe" @('-D', "`"$stateRoot\pg`"", '-h', '127.0.0.1', '-p', '15433')
Wait-Port 15433
Run-Psql 'postgres' "CREATE ROLE avrag LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOINHERIT PASSWORD 'avrag'; CREATE ROLE avrag_runtime LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOINHERIT PASSWORD 'avrag';"
Run-Psql 'postgres' 'CREATE DATABASE avrag_client OWNER avrag;'
Run-Psql 'avrag_client' 'CREATE EXTENSION IF NOT EXISTS vector;'
$null = Start-Tracked 'redis' "$InstallDir\runtime\redis\redis-server.exe" @('--bind','127.0.0.1','--port','16380','--dir',"`"$stateRoot\redis`"")
Wait-Port 16380
}
# Only model settings are reused. Identity, secrets and storage belong to this run.
foreach ($line in Get-Content (Join-Path $sourceRoot 'avrag-rs\.env')) {
    if ($line -match '^(AGENT_LLM_[A-Z_]+|PLATFORM_OFFICIAL_RATES_JSON)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2].Trim().Trim('"').Trim("'"), 'Process') }
}
$env:DATABASE_URL='postgres://avrag_runtime:avrag@127.0.0.1:15433/avrag_client'
$env:MIGRATION_DATABASE_URL='postgres://avrag:avrag@127.0.0.1:15433/avrag_client'
$env:REDIS_URL='redis://127.0.0.1:16380/0'
$env:REDIS_ADDR='127.0.0.1:16380'
$env:RETRIEVAL_BACKEND='pgvector'
$env:AVRAG_API_ADDR='127.0.0.1:18082'
$env:AVRAG_PUBLIC_BASE_URL='http://127.0.0.1:18082'
$env:CLIENT_API_BASE_URL=$env:AVRAG_PUBLIC_BASE_URL
$env:AVRAG_OBJECT_ROOT="$stateRoot\objects"
$env:PROMPT_DIR="$InstallDir\prompts"
$env:AVRAG_MIGRATIONS_DIR="$InstallDir\runtime\migrations"
$env:JWT_SECRET=[Guid]::NewGuid().ToString('N')+[Guid]::NewGuid().ToString('N')
$env:AVRAG_UPLOAD_SIGNING_SECRET=[Guid]::NewGuid().ToString('N')+[Guid]::NewGuid().ToString('N')
$env:BYOK_MASTER_KEY=[Convert]::ToBase64String([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
$env:AVRAG_PLATFORM_KEYS_RELAY='false'
$migration = Start-Tracked 'migrate' "$InstallDir\avrag-migrate.exe" @()
if (-not $migration.WaitForExit(60000)) { throw 'Migration still running; inspect tracked process before proceeding.' }
if ($migration.ExitCode -ne 0) { throw 'Migration failed; see logs.' }
Run-Psql 'avrag_client' @'
GRANT USAGE ON SCHEMA public TO avrag_runtime;
DO $$ DECLARE r record; BEGIN
FOR r IN SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename <> '_sqlx_migrations' AND tablename NOT LIKE '_org_owner_map%' LOOP
EXECUTE format('GRANT SELECT,INSERT,UPDATE,DELETE ON public.%I TO avrag_runtime',r.tablename);
END LOOP; END $$;
GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO avrag_runtime;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO avrag_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE avrag IN SCHEMA public GRANT SELECT,INSERT,UPDATE,DELETE ON TABLES TO avrag_runtime;
'@
$null = Start-Tracked 'api' "$InstallDir\avrag-api.exe" @()
Wait-Port 18082
$health=Invoke-RestMethod 'http://127.0.0.1:18082/health' -TimeoutSec 10
Write-Output "Isolated API healthy. State: $stateRoot"
$env:CONTEXT_OS_DESKTOP_DATA_DIR="$stateRoot\session"
$env:CONTEXT_OS_CLIENT_HOME=$stateRoot
# Interactive window requested for acceptance; no installed client is restarted.
Start-Process -FilePath $exe -WorkingDirectory $stateRoot
