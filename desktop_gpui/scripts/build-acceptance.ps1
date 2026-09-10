param([string]$Workspace = 'C:\dev\context-osv6')
$ErrorActionPreference = 'Stop'
$project = Join-Path ([IO.Path]::GetFullPath($Workspace)) 'desktop_gpui'
$target = Join-Path $project 'target'
$name = 'desktop-gpui-acceptance-' + (Get-Date -Format 'yyyyMMdd-HHmmss')
$runDir = Join-Path $target ('acceptance\build\' + $name)
New-Item -ItemType Directory -Path $runDir | Out-Null
$manifestPath = Join-Path $project 'Cargo.toml'
$manifest = Get-Content -Raw -LiteralPath $manifestPath
# Use the actual source and locked dependencies, with a distinct binary name so a
# running development client never needs to close or have its executable replaced.
$manifest = [regex]::Replace($manifest, 'path = "([^"]+)"', {
    param($match)
    $absolute = [IO.Path]::GetFullPath((Join-Path $project $match.Groups[1].Value)).Replace('\', '/')
    'path = "' + $absolute + '"'
})
$manifest = $manifest.Replace("[[bin]]`r`n", "[[bin]]`n")
$manifest = $manifest.Replace("[[bin]]`nname = `"desktop-gpui`"", "[[bin]]`nname = `"$name`"")
if (-not $manifest.Contains("name = `"$name`"")) { throw 'GPUI binary target was not found.' }
$lib = (Join-Path $project 'src\lib.rs').Replace('\', '/')
$manifest += "`n[lib]`npath = `"$lib`"`n`n[workspace]`n"
Set-Content -LiteralPath (Join-Path $runDir 'Cargo.toml') -Value $manifest -Encoding utf8
Copy-Item -LiteralPath (Join-Path $project 'Cargo.lock') -Destination (Join-Path $runDir 'Cargo.lock')
$liveBinary = Join-Path $target 'debug\desktop-gpui.exe'
$before = if (Test-Path -LiteralPath $liveBinary) { (Get-FileHash -LiteralPath $liveBinary).Hash } else { $null }
$previousJobs = $env:CARGO_BUILD_JOBS
$started = Get-Date
try {
    $env:CARGO_BUILD_JOBS = '2'
    & cargo build --locked --manifest-path (Join-Path $runDir 'Cargo.toml') --target-dir $target --features ui --bin $name *> (Join-Path $runDir 'build.log')
    if ($LASTEXITCODE -ne 0) { throw "Build failed; see $runDir\build.log" }
    $artifact = Join-Path $target ("debug\$name.exe")
    if (-not (Test-Path -LiteralPath $artifact)) { throw 'Build did not produce the acceptance executable.' }
    if ($before -and (Get-FileHash -LiteralPath $liveBinary).Hash -ne $before) { throw 'Development binary changed unexpectedly.' }
    $hashes = [ordered]@{}
    foreach ($file in @((Get-Item -LiteralPath $manifestPath), (Get-Item -LiteralPath (Join-Path $project 'Cargo.lock'))) + @(Get-ChildItem -LiteralPath (Join-Path $project 'src') -Recurse -File)) {
        $hashes[[IO.Path]::GetRelativePath($project, $file.FullName)] = (Get-FileHash -LiteralPath $file.FullName).Hash
    }
    [ordered]@{
        ok = $true; artifact = $artifact; sha256 = (Get-FileHash -LiteralPath $artifact).Hash
        seconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 2)
        sourceHashes = $hashes; developmentBinaryUnchanged = $true
        note = 'Production ui feature; actual source and lockfile; acceptance manifest only changes source paths and binary name. Not launched.'
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $runDir 'result.json') -Encoding utf8
    Get-Content -LiteralPath (Join-Path $runDir 'result.json')
} finally {
    $env:CARGO_BUILD_JOBS = $previousJobs
}
