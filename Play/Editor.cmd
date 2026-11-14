@echo off
cd /d "%~dp0.."
rem The editor. Leave it open: it runs from a copy, so the engine can be
rem rebuilt while you watch, and it restarts itself into each new build
rem (keeping your scene). Scenes, materials and assets edited on disk reload live.
cargo build --release --bin grave-engine
if errorlevel 1 (
  echo.
  echo The editor could not be built.
  pause
  exit /b 1
)
start "" "target\release\grave-engine.exe" %*
