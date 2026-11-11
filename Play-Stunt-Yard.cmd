@echo off
cd /d "%~dp0"
rem Build first (quick when nothing changed) so you always play the latest version.
cargo build --release --bin stunt-yard
if errorlevel 1 (
  echo.
  echo The game could not be built. If Stunt Yard is already open, close it and try again.
  pause
  exit /b 1
)
start "" "targetelease\stunt-yard.exe"
