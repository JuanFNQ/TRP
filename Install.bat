@echo off
rem TRP's installer: run once (double-click). Afterwards Play.bat starts TRP.
rem   1. the C++ build tools and Rust, each offered through winget where it is missing
rem   2. TRP built (cargo build --release)
rem   3. where the games are asked (trp.exe --setup-only)
setlocal
title TRP - Install
cd /d "%~dp0TRP" || goto :no_folder

echo.
echo  TRP - Install
echo  =============
echo  Sets TRP up once. After this, double-click Play.bat to play.
echo.
echo   1. Checks for the C++ build tools and Rust, and offers to install what is missing.
echo   2. Builds TRP (several minutes the first time).
echo   3. Asks where your SPT folder is.
echo.

rem The C++ build tools before Rust: Rust's own installer asks for them.
call :find_cpp
if defined VCTOOLS goto :cpp_ok
echo  [1/3] The Visual Studio C++ build tools are not installed. TRP needs them to build.
call :can_winget || goto :cpp_by_hand
choice /c YN /m "       Install them now (Visual Studio Build Tools 2022 with C++, about 5 GB)"
if errorlevel 2 goto :cpp_by_hand
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-source-agreements --accept-package-agreements --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
call :find_cpp
if not defined VCTOOLS goto :cpp_by_hand
:cpp_ok
echo  [1/3] C++ build tools: found.

call :find_cargo
if defined CARGO goto :rust_ok
echo  [1/3] Rust is not installed.
call :can_winget || goto :rust_by_hand
choice /c YN /m "       Install it now (rustup, from rust-lang.org)"
if errorlevel 2 goto :rust_by_hand
winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
call :find_cargo
if not defined CARGO goto :rust_by_hand
:rust_ok
echo  [1/3] Rust: found.

echo.
echo  [2/3] Building TRP. The first time takes several minutes: it builds the Bevy engine and PhysX.
echo.
cargo build --release -p trp-app
if errorlevel 1 goto :build_failed

echo.
echo  [3/3] Where your games are.
cargo run --release -q -p trp-app -- --setup-only
if errorlevel 1 goto :setup_failed

echo  Done. Double-click Play.bat to play.
echo.
pause
exit /b 0

:cpp_by_hand
echo.
echo  Install the C++ build tools by hand, then run Install.bat again:
echo    https://visualstudio.microsoft.com/visual-cpp-build-tools/
echo    In its installer, tick "Desktop development with C++".
start "" "https://visualstudio.microsoft.com/visual-cpp-build-tools/"
goto :stop

:rust_by_hand
echo.
echo  Install Rust by hand, then run Install.bat again:
echo    https://rustup.rs
start "" "https://rustup.rs"
goto :stop

:build_failed
echo.
echo  The build failed: the lines above say why.
goto :stop

:setup_failed
echo.
echo  TRP could not keep where your games are: the lines above say why.
goto :stop

:no_folder
echo  Install.bat has to stay beside the TRP folder it came with.

:stop
echo.
pause
exit /b 1

rem VCTOOLS: where Visual Studio's C++ tools are, if anywhere.
:find_cpp
set "VCTOOLS="
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" exit /b 0
for /f "usebackq delims=" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VCTOOLS=%%i"
exit /b 0

rem CARGO: set when cargo runs (a console opened before Rust was installed has no path to it yet).
:find_cargo
set "CARGO="
where cargo >nul 2>nul || if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>nul || exit /b 0
cargo --version >nul 2>nul && goto :cargo_found
echo        Rust has no toolchain yet: getting the stable one.
rustup default stable
cargo --version >nul 2>nul || exit /b 0
:cargo_found
set "CARGO=1"
exit /b 0

:can_winget
where winget >nul 2>nul
exit /b %errorlevel%
