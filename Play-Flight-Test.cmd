@echo off
cd /d "%~dp0"
cargo build --release --bin flight-test
if errorlevel 1 (
  echo Close an existing flight lab window if its program is locked, then try again.
  pause
  exit /b 1
)
start "" "target\release\flight-test.exe"
