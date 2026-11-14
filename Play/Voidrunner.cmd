@echo off
cd /d "%~dp0.."
rem Build first (quick when nothing changed) so you always play the latest version.
cargo build --release --bin voidrunner
if errorlevel 1 (
  echo.
  echo Voidrunner could not be built. If it is already open, close it and try again.
  pause
  exit /b 1
)
start "" "target\release\voidrunner.exe"
