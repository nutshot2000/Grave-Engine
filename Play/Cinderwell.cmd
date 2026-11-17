@echo off
cd /d "%~dp0.."
cargo build --release --bin cinderwell
if errorlevel 1 (
  pause
  exit /b 1
)
start "" "target\release\cinderwell.exe"
