@echo off
cd /d "%~dp0"
if not exist "target\release\dust-hawk.exe" (
  cargo build --release --bin dust-hawk
  if errorlevel 1 (
    pause
    exit /b 1
  )
)
start "" "target\release\dust-hawk.exe"
