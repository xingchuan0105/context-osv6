# Context-OS desktop — Windows host env bootstrap for `pnpm tauri dev`.
# Run in PowerShell (ExecutionPolicy Bypass if needed):
#   powershell -ExecutionPolicy Bypass -File \\wsl$\Ubuntu\home\chuan\context-osv6\scripts\dev-windows-env.ps1
#
# Creates a drive-letter workspace (UNC is hostile to npm/cargo), refreshes PATH,
# installs pnpm if missing, then optionally starts `pnpm tauri dev`.

$ErrorActionPreference = "Stop"

# --- PATH: Node + Cargo + npm global ---
$env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
            [System.Environment]::GetEnvironmentVariable("Path", "User")
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) { $env:Path = "$cargoBin;$env:Path" }
$npmGlobal = Join-Path $env:APPDATA "npm"
if (Test-Path $npmGlobal) { $env:Path = "$npmGlobal;$env:Path" }

function Require-Cmd($name) {
  if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
    throw "Missing '$name' on PATH. Install via winget (Node LTS / Rustup / VS Build Tools)."
  }
}

Require-Cmd node
Require-Cmd cargo

# pnpm
if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
  Write-Host "Installing pnpm (user)…"
  & npm.cmd install -g pnpm
  $env:Path = "$npmGlobal;$env:Path"
}
Require-Cmd pnpm

# --- Workspace on a real drive letter (not UNC \\wsl$\) ---
$WslRepo = "\\wsl$\Ubuntu\home\chuan\context-osv6"
$WinRoot = "C:\dev\context-osv6"
if (-not (Test-Path "C:\dev")) {
  New-Item -ItemType Directory -Path "C:\dev" | Out-Null
}

if (-not (Test-Path $WinRoot)) {
  if (Test-Path $WslRepo) {
    Write-Host "Creating junction $WinRoot -> $WslRepo"
    cmd /c mklink /J "$WinRoot" "$WslRepo"
  } else {
    throw "Neither $WinRoot nor $WslRepo found. Clone the repo to C:\dev\context-osv6."
  }
}

Set-Location (Join-Path $WinRoot "desktop")
Write-Host "cwd=$(Get-Location)"

if (-not (Test-Path "node_modules")) {
  Write-Host "pnpm install (desktop)…"
  pnpm install
}
if (-not (Test-Path (Join-Path $WinRoot "frontend_next\node_modules"))) {
  Write-Host "pnpm install (frontend_next)…"
  Push-Location (Join-Path $WinRoot "frontend_next")
  pnpm install
  Pop-Location
}

# MSVC check
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
  $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  Write-Host "MSVC tools: $vs"
} else {
  Write-Host "WARNING: VS Build Tools not detected. Install:"
  Write-Host '  winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"'
}

# Smart App Control blocks cargo build-script EXEs (os error 4551) when enabled.
$sac = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy' -Name VerifiedAndReputablePolicyState -EA SilentlyContinue).VerifiedAndReputablePolicyState
if ($sac -eq 1 -or $sac -eq 2) {
  Write-Host ""
  Write-Host "WARNING: Smart App Control is ON (state=$sac)."
  Write-Host "  cargo build will fail with os error 4551 on build-script-build."
  Write-Host "  Turn OFF: Settings → Privacy & security → Windows Security →"
  Write-Host "            App & browser control → Smart App Control → Off"
  Write-Host "  (may require reboot). Until then use WSL hotswap:"
  Write-Host "    wsl -d Ubuntu -- bash -lc 'cd /home/chuan/context-osv6 && bash scripts/dev-windows-hotswap.sh shell-only'"
}

Write-Host ""
Write-Host "Ready. Start dev with:"
Write-Host "  cd $WinRoot\desktop"
Write-Host "  pnpm tauri dev"
Write-Host ""
Write-Host "Hot-swap shell from WSL (no NSIS):"
Write-Host "  wsl -d Ubuntu -- bash -lc 'cd /home/chuan/context-osv6 && bash scripts/dev-windows-hotswap.sh shell-only'"

if ($env:START_TAURI_DEV -eq "1") {
  Write-Host "START_TAURI_DEV=1 — launching pnpm tauri dev…"
  pnpm tauri dev
}
