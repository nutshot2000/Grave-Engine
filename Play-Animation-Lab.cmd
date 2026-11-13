@echo off
cd /d "%~dp0"
if not exist "scenes\animation-lab.local.txt" (
  py tools\build_mixamo_lab.py --render-only
  if errorlevel 1 (
    pause
    exit /b 1
  )
)
cargo build --release --bin grave-engine
if errorlevel 1 (
  pause
  exit /b 1
)
start "" "target\release\grave-engine.exe" "scenes\animation-lab.local.txt"
