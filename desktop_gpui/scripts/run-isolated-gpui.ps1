param(
    [string]$StateDir = 'C:\dev\gpui-acceptance-20260909',
    [string]$Workspace = 'C:\dev\context-osv6'
)
$ErrorActionPreference = 'Stop'
$stateRoot = (Resolve-Path -LiteralPath $StateDir).Path
$exe = Join-Path $Workspace 'desktop_gpui\target\debug\desktop-gpui.exe'
$sessionDir = Join-Path $stateRoot 'session-current'
if (-not (Test-Path -LiteralPath $exe) -or -not (Test-Path -LiteralPath $sessionDir)) {
    throw 'The built GPUI client and isolated current session must already exist.'
}
# This launcher only opens the client. It never starts or refreshes the API/data services.
$tracked = Get-Content -LiteralPath (Join-Path $stateRoot 'processes.jsonl') | ForEach-Object { $_ | ConvertFrom-Json }
$api = $tracked | Where-Object name -eq 'api' | Select-Object -Last 1
$listener = Get-NetTCPConnection -State Listen -LocalPort 18082 -ErrorAction Stop
$process = Get-CimInstance Win32_Process -Filter "ProcessId = $($api.id)"
if ($listener.OwningProcess -ne $api.id -or $process.ExecutablePath -ne $api.path) {
    throw 'The isolated API process does not match its ownership record; no service was changed.'
}
$null = Invoke-RestMethod 'http://127.0.0.1:18082/health' -TimeoutSec 3
$env:CLIENT_API_BASE_URL = 'http://127.0.0.1:18082'
$env:AVRAG_PUBLIC_BASE_URL = $env:CLIENT_API_BASE_URL
$env:CONTEXT_OS_CLIENT_HOME = $stateRoot
$env:CONTEXT_OS_DESKTOP_DATA_DIR = $sessionDir
$client = Start-Process -FilePath $exe -WorkingDirectory $stateRoot -PassThru
Write-Output "GPUI opened (PID $($client.Id)); existing isolated API and session reused."
