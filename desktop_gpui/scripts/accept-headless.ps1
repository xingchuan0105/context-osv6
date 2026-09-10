param(
    [string]$Workspace = 'C:\dev\context-osv6',
    [int]$Port = 18181,
    [ValidateRange(1, 20)][int]$Iterations = 1
)
$ErrorActionPreference = 'Stop'
if ($Port -le 1024 -or $Port -in @(18080, 18081, 18082)) { throw 'Use an isolated fixture port.' }
$project = Join-Path $Workspace 'desktop_gpui'
$logs = Join-Path $project 'target\acceptance\headless'
New-Item -ItemType Directory -Force $logs | Out-Null
$runDir = Join-Path $logs ('gpui-ui-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $runDir | Out-Null
$settings = @{
    CARGO_BUILD_JOBS = '2'
    CARGO_TARGET_DIR = (Join-Path $project 'target')
    CLIENT_API_BASE_URL = "http://127.0.0.1:$Port"
    AVRAG_PUBLIC_BASE_URL = "http://127.0.0.1:$Port"
    CONTEXT_OS_CLIENT_HOME = $runDir
    CONTEXT_OS_DESKTOP_DATA_DIR = $runDir
    GPUI_ACCEPTANCE_UI = '1'
    ITERATIONS = $Iterations.ToString()
    SEED = '0'
}
$previous = @{}
foreach ($key in $settings.Keys) {
    $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
    [Environment]::SetEnvironmentVariable($key, $settings[$key], 'Process')
}
$started = Get-Date
$exitCode = 1
Push-Location $project
try {
    & cargo test --locked --features headless-tests --bin desktop-gpui -- --test-threads=1 *> (Join-Path $runDir 'tests.log')
    $exitCode = $LASTEXITCODE
    $output = Get-Content -Raw -LiteralPath (Join-Path $runDir 'tests.log')
    $match = [regex]::Match($output, 'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored')
    $passed = if ($match.Success) { [int]$match.Groups[2].Value } else { 0 }
    $failed = if ($match.Success) { [int]$match.Groups[3].Value } else { 0 }
    $ignored = if ($match.Success) { [int]$match.Groups[4].Value } else { 0 }
    $ok = $exitCode -eq 0 -and $match.Success -and $passed -gt 0 -and $failed -eq 0 -and $ignored -eq 0
    $hashes = [ordered]@{}
    foreach ($file in @('Cargo.lock', 'src/main.rs', 'src/ui.rs', 'src/shell_view.rs', 'src/chat_view.rs', 'src/visual_preview.rs', 'src/service_view.rs', 'src/ui_tests.rs', 'src/ui_tests/fixture.rs', 'src/ui_tests/knowledge_fixture.rs', 'src/ui_tests/knowledge_tests.rs', 'src/ui_tests/presentation_tests.rs', 'src/runtime.rs', 'src/services.rs', 'src/session.rs', 'src/workspace.rs', 'src/knowledge_view.rs', 'src/knowledge_render.rs', '../desktop/core/src/api_proxy.rs', '../desktop/core/src/local_product.rs', '../desktop/core/src/local_stack.rs', '../desktop/core/src/native_stack.rs', '../desktop/core/src/process_deadline.rs', '../desktop/core/src/runtime_lease.rs', '../desktop/core/src/runtime_ports.rs', '../desktop/core/src/win_cmd.rs')) {
        $hashes[$file] = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $project $file)).Hash
    }
    [ordered]@{
        suite = 'GPUI headless UI'; passed = $passed; failed = $failed; ignored = $ignored
        ok = $ok; exitCode = $exitCode; seconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 2)
        api = $settings.CLIENT_API_BASE_URL; runDirectory = $runDir
        command = 'cargo test --locked --features headless-tests --bin desktop-gpui -- --test-threads=1'
        sourceHashes = $hashes
        iterationsPerTest = $Iterations
        coverage = 'Real GPUI views/events, real Host, synthetic loopback HTTP; no OS desktop interaction or paid models'
        notCovered = 'GPU pixels, system IME candidates, managed product cold start/migrations'
    } | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $runDir 'result.json')
    Get-Content -LiteralPath (Join-Path $runDir 'result.json')
    if (-not $ok) { throw "Headless acceptance failed; see $runDir\tests.log" }
} finally {
    Pop-Location
    foreach ($key in $settings.Keys) {
        [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process')
    }
}
