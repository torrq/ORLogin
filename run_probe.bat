@echo off
echo =========================================================
echo   Running OSRO Memory Probe (Elevated)
echo =========================================================
powershell -Command "Start-Process '%~dp0build\Release\probe.exe' -WorkingDirectory '%~dp0' -Verb RunAs -Wait"
if exist "%~dp0probe_out.txt" (
    echo.
    type "%~dp0probe_out.txt"
) else (
    echo [ERROR] probe_out.txt was not generated.
)
