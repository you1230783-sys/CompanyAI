@echo off
call "%~1\VC\Auxiliary\Build\vcvars64.bat" -vcvars_ver=14.2 10.0.19041.0 >nul
if errorlevel 1 exit /b 1
set
