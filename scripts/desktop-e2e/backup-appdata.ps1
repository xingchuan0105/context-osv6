[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("Backup", "Restore")]
    [string]$Action,

    [Parameter(Mandatory = $true)]
    [string]$BackupPath
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$appData = Join-Path $env:APPDATA "com.contextos.desktop"
$backupDir = Join-Path $BackupPath "appdata"
$names = @("local_user.json", "local_session.json", "llm-config.json")

if ($Action -eq "Backup") {
    New-Item -ItemType Directory -Force -Path $backupDir | Out-Null
    $manifest = [ordered]@{}
    foreach ($name in $names) {
        $source = Join-Path $appData $name
        if (Test-Path $source) {
            Copy-Item -Force -Path $source -Destination (Join-Path $backupDir $name)
            $manifest[$name] = "present"
        } else {
            $manifest[$name] = "absent"
        }
    }
    $manifest | ConvertTo-Json | Set-Content -Encoding UTF8 -Path (Join-Path $backupDir "manifest.json")
    Write-Output "APP_DATA_BACKUP=$backupDir"
    exit 0
}

if (-not (Test-Path $backupDir)) {
    Write-Output "APP_DATA_BACKUP_MISSING=$backupDir"
    exit 0
}

$manifestPath = Join-Path $backupDir "manifest.json"
if (-not (Test-Path $manifestPath)) {
    throw "AppData backup manifest missing at $manifestPath"
}
$manifest = Get-Content -Raw -Path $manifestPath | ConvertFrom-Json
New-Item -ItemType Directory -Force -Path $appData | Out-Null

foreach ($name in $names) {
    $property = $manifest.PSObject.Properties[$name]
    $state = if ($null -ne $property) { [string]$property.Value } else { "absent" }
    $backupFile = Join-Path $backupDir $name
    $destination = Join-Path $appData $name
    if ($state -eq "present") {
        if (-not (Test-Path $backupFile)) {
            throw "AppData backup file missing for $name"
        }
        Copy-Item -Force -Path $backupFile -Destination (Join-Path $appData $name)
    } elseif (Test-Path $destination) {
        Remove-Item -Force -Path $destination
    }
}

Write-Output "APP_DATA_RESTORED=$backupDir"
exit 0
