@echo off
cd /d "%~dp0.."
cargo build --release --bin cave-explorer
if errorlevel 1 (
  echo The cave expedition could not be built.
  pause
  exit /b 1
)
start "" "target\release\cave-explorer.exe"
