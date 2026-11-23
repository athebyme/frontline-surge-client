@echo off
setlocal
if not exist "%~dp0Client\FrontlineSurge.exe" goto incomplete
if not exist "%~dp0Server\FrontlineServer.exe" goto incomplete
start "Frontline Surge - Dedicated Server" /D "%~dp0Server" "%~dp0Server\FrontlineServer.exe" --bind 127.0.0.1 --port 8787 --bots 4 --dev-local
start "" /D "%~dp0Client" "%~dp0Client\FrontlineSurge.exe"
exit /b 0
:incomplete
echo Extract the whole ZIP before starting Play.cmd. Keep the Client and Server folders beside it.
pause
exit /b 1
