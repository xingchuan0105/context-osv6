[CmdletBinding()]
param(
    [ValidateSet("Seed", "Remove")]
    [string]$Action = "Seed",
    [string]$AppDataPath = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ([string]::IsNullOrWhiteSpace($AppDataPath)) {
    $AppDataPath = Join-Path $env:APPDATA "com.contextos.desktop"
}

$target = Join-Path $AppDataPath "llm-config.json"

if ($Action -eq "Remove") {
    if (Test-Path $target) {
        Remove-Item -Force -Path $target
    }
    Write-Output "LEGACY_LLM_REMOVED=$target"
    exit 0
}

New-Item -ItemType Directory -Force -Path $AppDataPath | Out-Null
$config = [ordered]@{
    provider = "custom"
    base_url = "http://127.0.0.1:9"
    api_key = "e2e-not-a-real-key"
    model = "e2e-dummy"
    timeout_ms = 2000
    enable_thinking = $null
    enable_cache = $null
    embedding = $null
}
$json = $config | ConvertTo-Json -Depth 6
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($target, $json, $utf8NoBom)

Write-Output "LEGACY_LLM_SEEDED=$target"
exit 0
