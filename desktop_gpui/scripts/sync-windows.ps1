param([string]$Destination = 'C:\dev\context-osv6')
$ErrorActionPreference = 'Stop'
$source = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$destinationRoot = [IO.Path]::GetFullPath($Destination)
foreach ($part in @('desktop_gpui', 'desktop\core', 'contracts', 'frontend_rust\crates\web-sdk', 'frontend_rust\tests\fixtures')) {
    $from = Join-Path $source $part
    $to = Join-Path $destinationRoot $part
    # Copy only; no /MIR, /PURGE or deletion of target/cache directories.
    & robocopy $from $to /E /XD target node_modules .git /XF .env /NFL /NDL /NJH /NJS /NP
    if ($LASTEXITCODE -ge 8) { throw "Copy failed: $part" }
}
Copy-Item -LiteralPath (Join-Path $source 'frontend_rust\Cargo.toml') -Destination (Join-Path $destinationRoot 'frontend_rust\Cargo.toml')
Write-Output "GPUI sources synced to $destinationRoot"
