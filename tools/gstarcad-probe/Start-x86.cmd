@echo off
chcp 65001 >nul
"%~dp0runtime-x86\python.exe" -I "%~dp0probe.py" %*
pause
