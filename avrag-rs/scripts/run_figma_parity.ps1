param(
  [string]$BaseUrl = "http://127.0.0.1:4173",
  [double]$Threshold = 0.02,
  [switch]$WriteDiffImages
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
$projectRoot = Resolve-Path (Join-Path $repoRoot "..")
$visualRoot = Join-Path $projectRoot "frontend_rust/.run/visual_compare"
$expectedDir = Join-Path $visualRoot "figma"
$actualDir = Join-Path $visualRoot "playwright"
$analysisDir = Join-Path $visualRoot "analysis"

Write-Host "[parity] capture playwright screenshots..." -ForegroundColor Cyan
$env:PARITY_BASE_URL = $BaseUrl
$env:PARITY_PLAYWRIGHT_DIR = $actualDir
python (Join-Path $repoRoot "scripts/capture_preview_pages.py")
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

if (!(Test-Path $expectedDir)) {
  Write-Error "[parity] missing expected figma screenshots: $expectedDir"
  exit 2
}

$expectedCount = (Get-ChildItem $expectedDir -Filter *.png -ErrorAction SilentlyContinue | Measure-Object).Count
if ($expectedCount -eq 0) {
  Write-Error "[parity] no expected png files in: $expectedDir"
  exit 2
}

Write-Host "[parity] compare figma vs playwright..." -ForegroundColor Cyan
$pyArgs = @(
  (Join-Path $repoRoot "scripts/compare_figma_playwright.py"),
  "--expected-dir", $expectedDir,
  "--actual-dir", $actualDir,
  "--out-dir", $analysisDir,
  "--threshold", "$Threshold"
)
if ($WriteDiffImages) {
  $pyArgs += "--write-diff-images"
}
python @pyArgs
$exitCode = $LASTEXITCODE

$summaryPath = Join-Path $analysisDir "summary.json"
$reportPath = Join-Path $analysisDir "report.md"
Write-Host "[parity] summary: $summaryPath"
Write-Host "[parity] report:  $reportPath"

exit $exitCode
