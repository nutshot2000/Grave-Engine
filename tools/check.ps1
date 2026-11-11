# Check everything in one go: build, run every test, then simulate every
# scene headless and save a screenshot of each into shots\check\.
# Look at the screenshots afterwards: they show whether things look right.
#
#   tools\check.ps1            (from the project folder)
#   tools\check.ps1 -Frames 600
#
# Closes the editor first if it's open (it locks the program file).
param([int]$Frames = 300)
Set-Location (Join-Path $PSScriptRoot "..")
Stop-Process -Name grave-engine -ErrorAction SilentlyContinue

Write-Host "== Building"
cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Host "BUILD FAILED"; exit 1 }

Write-Host "== Tests"
$tests = cargo test --release 2>&1 | Out-String
$tests -split "`n" | Where-Object { $_ -match "^test result|panicked|FAILED" } | ForEach-Object { Write-Host "  $_" }
$testsOk = -not ($tests -cmatch "test result: FAILED|panicked at")

Write-Host "== Scenes ($Frames frames each)"
New-Item -ItemType Directory -Force shots\check | Out-Null
$problems = 0
foreach ($scene in Get-ChildItem scenes\*.txt) {
    $name = $scene.BaseName
    $out = & (Join-Path $(if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }) 'release\grave-engine.exe') --run $scene.FullName --frames $Frames --scale 2 --shot "shots\check\$name.png" 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        $problems++
        Write-Host "  ${name}: PROBLEMS (exit $LASTEXITCODE)"
        $out -split "`n" | Where-Object { $_ -match "problem|error" } | ForEach-Object { Write-Host "      $_" }
    } else {
        $timing = ($out -split "`n" | Where-Object { $_ -match "ms/frame" }) -replace ".*\((.*ms/frame)\).*", '$1'
        $player = ($out -split "`n" | Where-Object { $_ -match "^Player" } | Select-Object -First 1)
        Write-Host "  ${name}: ok $timing $player"
    }
}
Write-Host ""
Write-Host "Tests: $(if ($testsOk) {'ok'} else {'FAILED'}). Scenes with problems: $problems. Screenshots: shots\check\"
if (-not $testsOk -or $problems -gt 0) { exit 1 }
