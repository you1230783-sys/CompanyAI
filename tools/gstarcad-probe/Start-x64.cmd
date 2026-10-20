@echo off
chcp 65001 >nul
"%~dp0runtime-x64\python.exe" -I "%~dp0probe.py" %*
pause
