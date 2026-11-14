@echo off
cd /d "%~dp0.."
py tools\prepare_climbing_lab.py --scene mobility-lab
if errorlevel 1 exit /b 1
cargo build --release --bin grave-engine
if errorlevel 1 exit /b 1
start "" "target\release\grave-engine.exe" "scenes\mobility-lab.local.txt"
