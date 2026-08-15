@echo off
setlocal EnableExtensions
cd /d "%SystemRoot%" >nul 2>&1
if "%~1"=="" exit /b 1

set "WSLPATH_IN=%~1"
set "WSLPATH_IN=%WSLPATH_IN:\=/%"
for /f "delims=" %%P in ('wsl.exe wslpath -u "%WSLPATH_IN%"') do set "WSLPATH_OUT=%%P"
if not defined WSLPATH_OUT exit /b 1

wsl.exe /home/chuan/.local/bin/markitdown "%WSLPATH_OUT%"
set "WSL_EXIT=%ERRORLEVEL%"
exit /b %WSL_EXIT%
