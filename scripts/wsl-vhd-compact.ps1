[CmdletBinding()]
param(
    [string]$DistroName = "Ubuntu",
    [string]$VhdPath,
    [switch]$Preview
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Resolve-VhdPath {
    param([string]$TargetDistro)

    $lxssRoots = @(
        "HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss",
        "HKLM:\Software\Microsoft\Windows\CurrentVersion\Lxss"
    )

    foreach ($root in $lxssRoots) {
        if (-not (Test-Path $root)) { continue }
        foreach ($key in Get-ChildItem $root) {
            $item = Get-ItemProperty $key.PSPath
            if ($item.DistributionName -ne $TargetDistro) { continue }
            if ($item.BasePath) {
                return (Join-Path $item.BasePath "ext4.vhdx")
            }
        }
    }

    throw "Could not resolve VHD path for distro '$TargetDistro'."
}

function Format-Size {
    param([long]$Bytes)
    if ($Bytes -ge 1GB) { return "{0:N2} GB" -f ($Bytes / 1GB) }
    if ($Bytes -ge 1MB) { return "{0:N2} MB" -f ($Bytes / 1MB) }
    return "$Bytes B"
}

function Invoke-WslCommand {
    param(
        [string[]]$Arguments,
        [switch]$WarnOnly
    )

    & wsl.exe @Arguments | Out-Host
    if ($LASTEXITCODE -eq 0) {
        return
    }

    $message = "wsl.exe $($Arguments -join ' ') failed with exit code $LASTEXITCODE."
    if ($WarnOnly) {
        Write-Warning $message
        return
    }

    throw $message
}

if (-not $VhdPath) {
    $VhdPath = Resolve-VhdPath -TargetDistro $DistroName
}

if (-not $VhdPath.EndsWith(".vhdx")) {
    throw "Refusing to compact a non-.vhdx path: $VhdPath"
}

Write-Host "Selected distro: $DistroName"
Write-Host "Selected VHD:    $VhdPath"

if ($Preview) {
    Write-Host "Preview mode: would run sparse enablement, WSL shutdown, and Optimize-VHD."
    return
}

if (-not (Test-IsAdministrator)) {
    throw "Administrator privileges are required. Re-run this script in an elevated PowerShell session."
}

if (-not (Get-Command Optimize-VHD -ErrorAction SilentlyContinue)) {
    throw "Optimize-VHD is unavailable. Enable the Hyper-V PowerShell module and retry."
}

Invoke-WslCommand -Arguments @("--manage", $DistroName, "--set-sparse", "true") -WarnOnly
Invoke-WslCommand -Arguments @("--shutdown")

if (-not (Test-Path $VhdPath)) {
    throw "VHD path not found: $VhdPath"
}

$before = (Get-Item $VhdPath).Length
Optimize-VHD -Path $VhdPath -Mode Full
$after = (Get-Item $VhdPath).Length

Write-Host ("Before: {0}" -f (Format-Size -Bytes $before))
Write-Host ("After:  {0}" -f (Format-Size -Bytes $after))
