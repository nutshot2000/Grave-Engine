@echo off
cd /d "%~dp0.."
rem The Millbrook waterwheel: press Play in the editor and watch the water turn it.
cargo build --release --bin grave-engine
if errorlevel 1 (
  echo.
  echo The editor could not be built.
  pause
  exit /b 1
)
start "" "target\release\grave-engine.exe" "scenes\millbrook-waterwheel.txt"
