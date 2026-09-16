@echo off
call "%~1\VC\Auxiliary\Build\vcvars64.bat" -vcvars_ver=14.29 %~2 >nul
if errorlevel 1 exit /b 1
set
