@echo off
setlocal
cd /d "%~dp0"
echo.
echo   SKELL STUDIO / QUICKVIEW
echo   Real native DAW screenshots. No mockup renderer.
echo.
if not exist "scripts\skell-quickview.ps1" (
  echo ERROR: scripts\skell-quickview.ps1 is missing.
  pause
  exit /b 1
)
if not "%~1"=="" (
  echo Capturing with the executable supplied by drag-and-drop:
  echo %~nx1
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File "scripts\skell-quickview.ps1" -Mode Render -Exe "%~f1" -Open
) else (
  echo Using existing preview EXE or building if missing.
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File "scripts\skell-quickview.ps1" -Mode Auto -Open
)
if errorlevel 1 (
  echo.
  echo QuickView did not complete. Read the error above.
  pause
  exit /b 1
)
exit /b 0
