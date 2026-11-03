# Re-evaluate the two reviewed RustSec alerts when dependencies/features change.
$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '../src-tauri')
try {
    $target = 'x86_64-pc-windows-msvc'
    $tree = & cargo tree --locked --target $target --format '{p}' --prefix none
    if ($LASTEXITCODE -ne 0) { throw 'Could not inspect the Windows dependency graph.' }
    foreach ($line in $tree) {
        if ($line -match '^glib v(0\.\d+\.\d+)') {
            $version = [version]$Matches[1]
            if ($version -ge [version]'0.15.0' -and $version -lt [version]'0.20.0') {
                throw 'RUSTSEC-2024-0429: affected glib is now compiled for Windows; re-evaluate the alert.'
            }
        }
    }
    $versions = $tree | ForEach-Object { if ($_ -match '^rand v(\d+\.\d+\.\d+)') { $Matches[1] } } | Sort-Object -Unique
    foreach ($text in $versions) {
        $version = [version]$text
        $affected = ($version -ge [version]'0.7.0' -and $version -lt [version]'0.8.6') -or ($version -ge [version]'0.9.0' -and $version -lt [version]'0.9.3') -or ($version -eq [version]'0.10.0')
        if ($affected) {
            $features = & cargo tree --locked --target $target -e features -i "rand@$text" --depth 1 --prefix none
            if ($LASTEXITCODE -ne 0) { throw 'Could not inspect rand features.' }
            if ($features -match '^rand feature "log"$') {
                throw 'RUSTSEC-2026-0097: affected rand now enables log; re-evaluate the alert.'
            }
        }
    }
    Write-Output 'Reviewed advisory scope valid: affected glib absent on Windows; affected rand has no log feature.'
} finally { Pop-Location }
