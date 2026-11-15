@echo off
cd /d "%~dp0.."
cargo build --release --bin grave-engine
if errorlevel 1 (
  pause
  exit /b 1
)
start "" "target\release\grave-engine.exe" "scenes\iron-heron-oilfield.txt"
