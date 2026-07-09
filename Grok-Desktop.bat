@echo off
setlocal
cd /d "%~dp0"
title Grok Desktop - Building production app (keep open)

echo.
echo ============================================================
echo  Grok Desktop - STABLE LAUNCH
echo ============================================================
echo.
echo  Raw cargo builds open http://127.0.0.1:1420
echo  That is why you saw ERR_CONNECTION_REFUSED or missing assets.
echo.
echo  This script uses Tauri's production build so build\
echo  is embedded and loads offline. First compile can take several minutes.
echo  KEEP THIS WINDOW OPEN until you see SUCCESS.
echo ============================================================
echo.

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\launch-stable.ps1"
set ERR=%ERRORLEVEL%

echo.
if not %ERR%==0 (
  echo LAUNCH FAILED with code %ERR%
  echo.
  pause
  exit /b %ERR%
)

echo If the Grok Desktop window shows the real app UI, you are done.
echo You can close this console; the app runs separately.
echo.
pause
exit /b 0
