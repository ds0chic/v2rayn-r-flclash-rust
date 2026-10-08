@echo off
rem One-click launcher for the acceptance restore GUI (test tool only).
setlocal
set PS=powershell.exe
"%PS%" -NoProfile -STA -ExecutionPolicy Bypass -File "%~dp0AcceptanceRestoreGui.ps1" %*
endlocal
