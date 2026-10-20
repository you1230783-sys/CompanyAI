@echo off
rem Open the compiled application in its isolated, offline tutorial window.
set "tutorial_exe=%~dp0target\x86_64-pc-windows-msvc\release\company-ai.exe"
if not exist "%tutorial_exe%" (
  echo Application not compiled. Run scripts\Build.ps1 -BuildOnly -EmptyCargoCache first.
  pause
  exit /b 1
)
start "" "%tutorial_exe%" --tutorial-preview
