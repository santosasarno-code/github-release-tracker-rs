@echo off
setlocal EnableExtensions
cd /d "%~dp0"
echo ==============================================
echo  GitHub Release Tracker (Rust) - build
echo ==============================================

where cargo >nul 2>nul
if errorlevel 1 (
    echo.
    echo [X] Rust is not installed ^(cargo not found^).
    echo     1. Install Rust from https://rustup.rs  ^(choose the default "MSVC" option^)
    echo     2. Install "Build Tools for Visual Studio" with the workload
    echo        "Desktop development with C++"  ^(it includes the Windows SDK^)
    echo     3. Open a NEW window and run this file again.
    goto :fail
)

rem --- version = the one in Cargo.toml (the only line that starts with: version = ) ---
set "VER="
for /f "tokens=2 delims==" %%A in ('findstr /b /c:"version = " Cargo.toml') do if not defined VER set "VER=%%A"
set "VER=%VER: =%"
set "VER=%VER:"=%"
echo %VER%| findstr /r /c:"^[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*$" >nul
if errorlevel 1 (
    echo [X] Could not read a version like 2.0.0 from Cargo.toml ^(got "%VER%"^).
    goto :fail
)
echo Version: %VER%   ^(the Git tag / release must be v%VER%^)
echo.

echo [1/3] Running the logic tests ...
cargo test --lib --locked --quiet
if errorlevel 1 (
    echo [X] Tests failed. Not building.
    goto :fail
)

echo.
echo [2/3] Building the exe ^(the first build downloads and compiles a few crates, 1-3 minutes^) ...
cargo build --release --locked
if errorlevel 1 (
    echo.
    echo [X] Build failed. Common causes:
    echo     - "linker `link.exe` not found"  -^> install Build Tools for Visual Studio, workload "Desktop development with C++"
    echo     - "rc.exe" / "RC.EXE not found"   -^> same workload, make sure a Windows 10/11 SDK is ticked
    echo     - no internet while downloading crates
    goto :fail
)

echo.
echo [3/3] Collecting the result ...
if not exist dist mkdir dist
copy /y "target\release\github-release-tracker.exe" "dist\GitHubReleaseTracker.exe" >nul
if errorlevel 1 (
    echo [X] Could not copy the exe.
    goto :fail
)
powershell -NoProfile -ExecutionPolicy Bypass -File "tools\release-notes.ps1" -Version "%VER%" -Exe "dist\GitHubReleaseTracker.exe" -Out "dist\release-notes-v%VER%.md"
if errorlevel 1 (
    echo [X] Could not write the release notes / hash.
    goto :fail
)

echo.
echo ==============================================
echo  DONE
echo   dist\GitHubReleaseTracker.exe
echo   dist\GitHubReleaseTracker.exe.sha256
echo   dist\release-notes-v%VER%.md   ^(paste into the GitHub release^)
echo ==============================================
for %%F in ("dist\GitHubReleaseTracker.exe") do echo  Size: %%~zF bytes
if not defined CI pause
exit /b 0

:fail
echo.
if not defined CI pause
exit /b 1
