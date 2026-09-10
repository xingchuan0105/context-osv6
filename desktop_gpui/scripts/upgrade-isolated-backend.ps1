param(
    [string]$StateDir = 'C:\dev\gpui-acceptance-20260909',
    [string]$BuildDir = 'C:\dev\gpui-backend-target\x86_64-pc-windows-gnu\debug',
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Context-OS Client'),
    [string]$Workspace = 'C:\dev\context-osv6',
    [switch]$Refresh
)
$ErrorActionPreference = 'Stop'
$stateRoot = [IO.Path]::GetFullPath($StateDir)
$sourceRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$backend = Join-Path $stateRoot 'backend-current'
$logs = Join-Path $stateRoot 'logs'
$pgBin = Join-Path $InstallDir 'runtime\pgsql\bin'
$tracked = Get-Content "$stateRoot\processes.jsonl" | ConvertFrom-Json
foreach ($name in @('postgres','redis')) {
    $entry = $tracked | Where-Object name -eq $name | Select-Object -Last 1
    $running = Get-CimInstance Win32_Process -Filter "ProcessId = $($entry.id)"
    if ($running.ExecutablePath -ne $entry.path -or -not $running.CommandLine.Contains($stateRoot)) { throw 'Isolated service ownership could not be verified.' }
}
foreach ($file in @('avrag-api.exe','avrag-migrate.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $BuildDir $file))) { throw "Missing current artifact: $file" }
}
if (-not $Refresh) {
if (Test-Path -LiteralPath $backend) { throw 'Current backend directory already exists; refusing to overwrite a running artifact.' }
New-Item -ItemType Directory -Path $backend | Out-Null
foreach ($file in @('avrag-api.exe','avrag-migrate.exe')) { Copy-Item -LiteralPath (Join-Path $BuildDir $file) -Destination $backend }
foreach ($file in @('libgcc_s_seh-1.dll','libstdc++-6.dll','libwinpthread-1.dll')) { Copy-Item -LiteralPath (Join-Path $InstallDir $file) -Destination $backend }
foreach ($part in @('prompts','modes','migrations')) { Copy-Item -LiteralPath (Join-Path $sourceRoot "avrag-rs\$part") -Destination $backend -Recurse }
}
function Sql([string]$Database,[string]$Statement) {
    & "$pgBin\psql.exe" -h 127.0.0.1 -p 15433 -U avrag_cluster_admin -d $Database -v ON_ERROR_STOP=1 -c $Statement *> "$logs\current-sql.log"
    if ($LASTEXITCODE -ne 0) { throw 'Isolated SQL failed; see current-sql.log' }
}
# The previous test database is retained. This database receives current migrations.
if (-not $Refresh) {
    Sql 'postgres' 'CREATE DATABASE avrag_gpui_current OWNER avrag;'
    Sql 'avrag_gpui_current' 'CREATE EXTENSION vector;'
}
foreach ($line in Get-Content (Join-Path $sourceRoot 'avrag-rs\.env')) {
    if ($line -match '^(AGENT_LLM_[A-Z_]+|PLATFORM_OFFICIAL_RATES_JSON)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2].Trim().Trim('"').Trim("'"), 'Process') }
}
$env:DATABASE_URL='postgres://avrag_runtime:avrag@127.0.0.1:15433/avrag_gpui_current'
$env:MIGRATION_DATABASE_URL='postgres://avrag:avrag@127.0.0.1:15433/avrag_gpui_current'
$env:REDIS_URL='redis://127.0.0.1:16380/1'
$env:REDIS_ADDR='127.0.0.1:16380'
$env:RETRIEVAL_BACKEND='pgvector'
$env:AVRAG_API_ADDR='127.0.0.1:18082'
$env:AVRAG_PUBLIC_BASE_URL='http://127.0.0.1:18082'
$env:CLIENT_API_BASE_URL=$env:AVRAG_PUBLIC_BASE_URL
$env:AVRAG_OBJECT_ROOT="$stateRoot\objects-current"
$env:PROMPT_DIR="$backend\prompts"
$env:AVRAG_MIGRATIONS_DIR="$backend\migrations"
$env:JWT_SECRET=[Guid]::NewGuid().ToString('N')+[Guid]::NewGuid().ToString('N')
$env:AVRAG_UPLOAD_SIGNING_SECRET=[Guid]::NewGuid().ToString('N')+[Guid]::NewGuid().ToString('N')
$env:BYOK_MASTER_KEY=[Convert]::ToBase64String([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
# Keep test identity stable across later configuration refreshes.
$secretPath=Join-Path $stateRoot 'current-secrets.json'
if (Test-Path -LiteralPath $secretPath) {
    $secrets=Get-Content -LiteralPath $secretPath -Raw | ConvertFrom-Json
    $env:JWT_SECRET=$secrets.jwt
    $env:AVRAG_UPLOAD_SIGNING_SECRET=$secrets.upload
    $env:BYOK_MASTER_KEY=$secrets.byok
} else {
    @{jwt=$env:JWT_SECRET;upload=$env:AVRAG_UPLOAD_SIGNING_SECRET;byok=$env:BYOK_MASTER_KEY} | ConvertTo-Json | Set-Content -LiteralPath $secretPath
    $acl=Get-Acl -LiteralPath $secretPath
    $acl.SetAccessRuleProtection($true,$false)
    $acl.SetAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.WindowsIdentity]::GetCurrent().User,'FullControl','Allow'))
    Set-Acl -LiteralPath $secretPath -AclObject $acl
}
$env:AVRAG_PLATFORM_KEYS_RELAY='false'
$env:RUST_LOG='warn'
function Start-Current([string]$Name,[string]$File) {
    $child=Start-Process -FilePath $File -WorkingDirectory $backend -WindowStyle Hidden -PassThru -RedirectStandardOutput "$logs\current-$Name.out.log" -RedirectStandardError "$logs\current-$Name.err.log"
    Add-Content -LiteralPath "$stateRoot\processes.jsonl" -Value (@{name=$Name;id=$child.Id;path=$File} | ConvertTo-Json -Compress)
    return $child
}
if (-not $Refresh) {
$migration=Start-Current 'migrate' "$backend\avrag-migrate.exe"
if (-not $migration.WaitForExit(60000)) { throw 'Migration still running; inspect tracked process.' }
if ($migration.ExitCode -ne 0) { throw 'Current migration failed; API has not been replaced.' }
Sql 'avrag_gpui_current' @'
GRANT USAGE ON SCHEMA public TO avrag_runtime;
DO $$ DECLARE r record; BEGIN
FOR r IN SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename <> '_sqlx_migrations' AND tablename NOT LIKE '_org_owner_map%' LOOP
EXECUTE format('GRANT SELECT,INSERT,UPDATE,DELETE ON public.%I TO avrag_runtime',r.tablename);
END LOOP; END $$;
GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO avrag_runtime;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO avrag_runtime;
'@
}
# Stop only the earlier API owned by this acceptance run, after migrations pass.
$oldApi=$tracked | Where-Object name -eq 'api' | Select-Object -Last 1
$listener=Get-NetTCPConnection -State Listen -LocalPort 18082 -ErrorAction Stop
$running=Get-CimInstance Win32_Process -Filter "ProcessId = $($oldApi.id)"
if ($listener.OwningProcess -ne $oldApi.id -or $running.ExecutablePath -ne $oldApi.path) { throw 'API ownership mismatch; no process stopped.' }
Stop-Process -Id $oldApi.id
$null=Start-Current 'api' "$backend\avrag-api.exe"
$ready=$false
for ($i=0; $i -lt 60; $i++) {
    try { $null=Invoke-RestMethod 'http://127.0.0.1:18082/health' -TimeoutSec 2; $ready=$true; break } catch { Start-Sleep -Milliseconds 500 }
}
if (-not $ready) { throw 'Current API health failed; inspect logs.' }
$env:CONTEXT_OS_DESKTOP_DATA_DIR="$stateRoot\session-current"
$env:CONTEXT_OS_CLIENT_HOME=$stateRoot
New-Item -ItemType Directory -Force -Path $env:CONTEXT_OS_DESKTOP_DATA_DIR | Out-Null
Start-Process -FilePath "$Workspace\desktop_gpui\target\debug\desktop-gpui.exe" -WorkingDirectory $stateRoot
Write-Output "Current API healthy at $env:CLIENT_API_BASE_URL; data remains isolated."
