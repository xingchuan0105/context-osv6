$ErrorActionPreference = "Continue"
$inst = Join-Path $env:LOCALAPPDATA "Context-OS Client"
$bin = Join-Path $inst "runtime\pgsql\bin"
$initdb = Join-Path $bin "initdb.exe"
$pgctl = Join-Path $bin "pg_ctl.exe"
$psql = Join-Path $bin "psql.exe"
$pgdata = Join-Path $env:TEMP "cos-pg-smoke-data"
$log = Join-Path $env:TEMP "cos-pg-smoke.log"

if (-not (Test-Path $initdb)) { Write-Host "FAIL missing $initdb"; exit 2 }
if (Test-Path $pgdata) { Remove-Item -Recurse -Force $pgdata }
New-Item -ItemType Directory -Force -Path $pgdata | Out-Null

$env:PATH = "$bin;" + $env:PATH
Write-Host "initdb..."
& $initdb -D $pgdata -U avrag --auth-local=trust --auth-host=trust --encoding=UTF8 -N
if ($LASTEXITCODE -ne 0) { Write-Host "FAIL initdb $LASTEXITCODE"; exit 3 }

$conf = Join-Path $pgdata "postgresql.conf"
Add-Content -Path $conf -Value "`nlisten_addresses = '127.0.0.1'`nport = 15433`n"

Write-Host "pg_ctl start..."
& $pgctl -D $pgdata -l $log -w start -o "-p 15433 -c listen_addresses=127.0.0.1"
if ($LASTEXITCODE -ne 0) {
  Write-Host "FAIL start $LASTEXITCODE"
  if (Test-Path $log) { Get-Content $log -Tail 40 }
  exit 4
}

$ver = & $psql -h 127.0.0.1 -p 15433 -U avrag -d postgres -tAc "SELECT version();"
Write-Host "version: $ver"

$extOut = & $psql -h 127.0.0.1 -p 15433 -U avrag -d postgres -c "CREATE EXTENSION IF NOT EXISTS vector;" 2>&1
Write-Host "create_extension: $extOut"

$extVer = & $psql -h 127.0.0.1 -p 15433 -U avrag -d postgres -tAc "SELECT extversion FROM pg_extension WHERE extname='vector';"
Write-Host "vector_version: $extVer"

& $pgctl -D $pgdata -m fast -w stop | Out-Null
if ($extVer -match "0\.") {
  Write-Host "PG_SMOKE_OK"
  exit 0
} else {
  Write-Host "PG_SMOKE_FAIL no vector"
  if (Test-Path $log) { Get-Content $log -Tail 40 }
  exit 5
}
