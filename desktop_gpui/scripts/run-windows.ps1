param(
    [string]$Workspace = 'C:\dev\context-osv6',
    [string]$RuntimeHome = (Join-Path $env:LOCALAPPDATA 'Context-OS Client')
)
$ErrorActionPreference = 'Stop'
$runtimeRoot = [IO.Path]::GetFullPath($RuntimeHome)
$exe = Join-Path $Workspace 'desktop_gpui\target\debug\desktop-gpui.exe'
foreach ($path in @($exe, (Join-Path $runtimeRoot 'avrag-api.exe'), (Join-Path $runtimeRoot 'avrag-worker.exe'), (Join-Path $runtimeRoot 'avrag-migrate.exe'), (Join-Path $runtimeRoot 'runtime\pgsql\bin\pg_ctl.exe'), (Join-Path $runtimeRoot 'client.env'))) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing Windows runtime file: $path" }
}
# Explicit Windows runtime selection; never reuse a copied WSL database tree.
$env:CONTEXT_OS_CLIENT_HOME = $runtimeRoot
$env:CONTEXT_OS_RUNTIME = Join-Path $runtimeRoot 'runtime'
$env:CLIENT_API_BASE_URL = 'http://127.0.0.1:18080'
$env:AVRAG_PUBLIC_BASE_URL = $env:CLIENT_API_BASE_URL
# This command opens the interactive client for native acceptance.
Start-Process -FilePath $exe -WorkingDirectory $runtimeRoot
