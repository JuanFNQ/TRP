@echo off
rem Starts TRP (double-click). It builds first only when the code has changed since the last
rem build, as after an update. Anything after Play.bat is passed on: Play.bat --equip
setlocal
title TRP
cd /d "%~dp0TRP" || goto :not_set_up
where cargo >nul 2>nul || if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>nul || goto :not_set_up
cargo run --release -p trp-app -- %*
if errorlevel 1 pause
exit /b

:not_set_up
echo  TRP is not set up yet: double-click Install.bat first.
pause
exit /b 1
