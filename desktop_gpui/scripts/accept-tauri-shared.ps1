param(
    [string]$Workspace = 'C:\dev\context-osv6',
    [switch]$WithHttpFixture,
    [switch]$WithHeadlessUi,
    [switch]$WithManagedProcesses
)
$ErrorActionPreference = 'Stop'
$project = Join-Path $Workspace 'desktop_gpui'
$logs = Join-Path $project 'target\acceptance\tauri-shared'
New-Item -ItemType Directory -Force $logs | Out-Null
$env:CARGO_BUILD_JOBS = '2'
$env:CARGO_TARGET_DIR = Join-Path $project 'target'
function Run-Cargo([string]$Name, [string[]]$CargoArgs) {
    & cargo @CargoArgs *> (Join-Path $logs "$Name.log")
    if ($LASTEXITCODE -ne 0) { throw "$Name failed; see $logs\$Name.log" }
    Get-Content (Join-Path $logs "$Name.log") | Select-String 'test result:'
}
Push-Location $project
try {
    Run-Cargo 'shared-core' @('test', '--manifest-path', '..\desktop\core\Cargo.toml', '--locked', '--lib')
    Run-Cargo 'gpui-and-original-stream' @('test', '--locked', '--lib', '--tests')
    if ($WithHttpFixture) {
        # The Rust test owns an in-process loopback fixture, no external app lifecycle.
        $env:CLIENT_API_BASE_URL = 'http://127.0.0.1:18180'
        $env:AVRAG_PUBLIC_BASE_URL = 'http://127.0.0.1:18180'
        $env:GPUI_ACCEPTANCE_HTTP = '1'
        Run-Cargo 'local-session-history' @('test', '--locked', '--lib', 'runtime::acceptance::local_session_and_history_over_http', '--', '--ignored', '--exact')
    }
    if ($WithHeadlessUi) {
        & (Join-Path $PSScriptRoot 'accept-headless.ps1') -Workspace $Workspace
    }
    if ($WithManagedProcesses) {
        & (Join-Path $PSScriptRoot 'accept-managed.ps1') -Workspace $Workspace
    }
    Write-Output "Requested host/UI suites passed. GPU pixels, system IME and real provider acceptance remain separate. Logs: $logs"
} finally {
    Pop-Location
}
