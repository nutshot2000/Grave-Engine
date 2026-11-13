@echo off
cd /d "%~dp0"
cargo build --release --bin dust-hawk
if errorlevel 1 (
  echo Could not update Dust Hawk. Close any running Dust Hawk window and try again.
  pause
  exit /b 1
)
start "" "target\release\dust-hawk.exe"
