param(
    [string]$Workspace = 'C:\dev\context-osv6',
    [string]$InstalledPython = (Join-Path $env:LOCALAPPDATA 'Context-OS Client\python')
)
$ErrorActionPreference = 'Stop'
$source = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$root = Join-Path $Workspace ('desktop_gpui\target\acceptance\office\gpui-office-' + [Guid]::NewGuid().ToString('N'))
if (-not (Test-Path -LiteralPath (Join-Path $InstalledPython 'python.exe'))) { throw 'Bundled Python missing.' }
$existing = @(Get-CimInstance Win32_Process | Where-Object { $_.Name -match '^(avrag-api|avrag-worker|desktop-gpui.*)\.exe$' } | Select-Object ProcessId, ExecutablePath, CreationDate)
New-Item -ItemType Directory -Path (Join-Path $root 'fixtures') -Force | Out-Null
Set-Content -LiteralPath (Join-Path $root 'acceptance.marker') -Value 'Isolated Windows Office package acceptance.'
$fixtures = @('frontend_rust\tests\fixtures\live-office\inventory.xlsx', 'frontend_rust\tests\fixtures\live-office\delivery.pptx', 'avrag-rs\crates\app\tests\product_e2e\fixtures\phase0-mini.docx')
foreach ($fixture in $fixtures) { Copy-Item -LiteralPath (Join-Path $source $fixture) -Destination (Join-Path $root 'fixtures') }
[IO.File]::WriteAllText((Join-Path $root 'fixtures\中文 资料.csv'), "货品,余量`n杉木,137`n", [Text.UTF8Encoding]::new($false))
Set-Content -LiteralPath (Join-Path $root 'fixtures\broken.docx') -Value 'This is not an Office ZIP package.'
$pythonDir = Join-Path $root 'staged\runtime\bin\python'
New-Item -ItemType Directory -Path $pythonDir -Force | Out-Null
Copy-Item -Path (Join-Path $InstalledPython '*') -Destination $pythonDir -Recurse
$python = Join-Path $pythonDir 'python.exe'
$started = Get-Date
$failure = $null
$savedReport = $env:GPUI_OFFICE_REPORT_DIR
try {
    & $python (Join-Path $source 'scripts\stage-desktop-office.py') --python-dir $pythonDir --parsers-dir (Join-Path $root 'staged\runtime\parsers') --wheel (Join-Path $root 'cache\firecrawl_anydoc-0.1.2-cp310-abi3-win_amd64.whl') *> (Join-Path $root 'stage.log')
    if ($LASTEXITCODE -ne 0) { throw 'Office package staging failed; see stage.log.' }
    # Reproduce NSIS resource mapping as a separate layout, without installing.
    New-Item -ItemType Directory -Path (Join-Path $root 'installed\runtime') -Force | Out-Null
    Copy-Item -LiteralPath $pythonDir -Destination (Join-Path $root 'installed\python') -Recurse
    Copy-Item -LiteralPath (Join-Path $root 'staged\runtime\parsers') -Destination (Join-Path $root 'installed\runtime\parsers') -Recurse
    $env:GPUI_OFFICE_REPORT_DIR = $root
    & $python (Join-Path $source 'scripts\desktop-e2e\test_office_parser.py') *> (Join-Path $root 'tests.log')
    if ($LASTEXITCODE -ne 0) { throw 'Office parser tests failed; see tests.log.' }
    $log = Get-Content -Raw -LiteralPath (Join-Path $root 'tests.log')
    if ($log -notmatch 'Ran 9 tests' -or $log -notmatch '(?m)^OK\s*$') { throw 'Incomplete parser test result.' }
} catch { $failure = $_.Exception.Message }
finally {
    $env:GPUI_OFFICE_REPORT_DIR = $savedReport
    foreach ($process in $existing) {
        $current = Get-CimInstance Win32_Process -Filter "ProcessId = $($process.ProcessId)"
        if (-not $current -or $current.ExecutablePath -ne $process.ExecutablePath -or $current.CreationDate -ne $process.CreationDate) { $failure = "Existing process changed: $($process.ProcessId). $failure" }
    }
    $files = @('scripts\stage-desktop-office.py', 'scripts\desktop-e2e\test_office_parser.py', 'desktop\runtime\parsers\anydoc-extract.cmd', 'avrag-rs\scripts\anydoc-extract\src\anydoc_extract\main.py') + $fixtures
    $hashes = [ordered]@{}
    foreach ($file in $files) { $hashes[$file] = (Get-FileHash -LiteralPath (Join-Path $source $file)).Hash }
    $result = [ordered]@{
        passed = ($null -eq $failure); error = $failure; root = $root
        seconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 2)
        sourceHashes = $hashes; preservedProcesses = $existing
        scope = 'Actual anydoc-extract command, embedded Python and pinned upstream Windows wheel; staged and installed layouts. No API, ingestion, retrieval, model call or desktop automation.'
    }
    $result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'result.json')
}
Get-Content -Raw -LiteralPath (Join-Path $root 'result.json')
if ($failure) { throw $failure }
