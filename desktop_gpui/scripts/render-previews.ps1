param([string]$Workspace = 'C:\dev\context-osv6', [int]$Port = 18183)
$ErrorActionPreference = 'Stop'
if ($Port -le 1024 -or $Port -in @(18080, 18081, 18082)) { throw 'Use an isolated fixture port.' }
$build = (& (Join-Path $PSScriptRoot 'build-acceptance.ps1') -Workspace $Workspace -VisualPreview | Out-String) | ConvertFrom-Json
if (-not $build.ok -or $build.features -ne 'headless-tests') { throw 'Visual build failed.' }
$root = Join-Path $Workspace ('desktop_gpui\target\acceptance\visual\gpui-ui-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$settings = @{
    CLIENT_API_BASE_URL = "http://127.0.0.1:$Port"
    AVRAG_PUBLIC_BASE_URL = "http://127.0.0.1:$Port"
    CONTEXT_OS_CLIENT_HOME = $root
    CONTEXT_OS_DESKTOP_DATA_DIR = $root
    GPUI_ACCEPTANCE_UI = '1'
}
$previous = @{}
foreach ($key in $settings.Keys) {
    $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
    [Environment]::SetEnvironmentVariable($key, $settings[$key], 'Process')
}
try {
    $process = Start-Process -FilePath $build.artifact -ArgumentList '--render-previews' -WorkingDirectory $root -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'stdout.log') -RedirectStandardError (Join-Path $root 'stderr.log')
    if (-not $process.WaitForExit(90000)) {
        # Only this script's dedicated capture process; never an existing client/service.
        $process.Kill($true)
        throw "Hidden render timed out; see $root"
    }
    if ($process.ExitCode -ne 0) { throw "Hidden render failed; see $root" }
    $result = Get-Content -Raw -LiteralPath (Join-Path $root 'visual-result.json') | ConvertFrom-Json
    if (-not $result.ok -or $result.files.Count -ne 24) { throw "Incomplete visual capture; see $root" }
    foreach ($file in $result.files) {
        if (-not (Test-Path -LiteralPath $file) -or (Get-Item -LiteralPath $file).Length -lt 1000) { throw "Missing capture: $file" }
    }
    $build | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'build-identity.json') -Encoding utf8
    Get-Content -LiteralPath (Join-Path $root 'visual-result.json')
} finally {
    foreach ($key in $settings.Keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process') }
}
