@echo off
setlocal
set "HERE=%~dp0"
if exist "%HERE%..\..\python\python.exe" (set "PY=%HERE%..\..\python\python.exe") else (set "PY=%HERE%..\bin\python\python.exe")
"%PY%" "%HERE%anydoc_lite.py" %*
exit /b %ERRORLEVEL%
