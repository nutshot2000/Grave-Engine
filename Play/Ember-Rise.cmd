@echo off
cd /d "%~dp0.."
cargo build --release --bin grave-engine
if errorlevel 1 (
  echo Could not rebuild Grave Engine. Close any open editor window and try again.
  pause
  exit /b 1
)
start "" "target\release\grave-engine.exe" "scenes\ember-rise.txt"
