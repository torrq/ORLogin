@echo off
setlocal enabledelayedexpansion

echo =========================================================
echo   Building ORLogin - Native C++ / Dear ImGui Login Helper
echo =========================================================

:: Locate Visual Studio
for /f "usebackq tokens=*" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do (
    set "VS_DIR=%%i"
)

if not defined VS_DIR (
    echo [ERROR] Visual Studio installation with C++ tools not found.
    exit /b 1
)

echo [*] Found Visual Studio: %VS_DIR%

:: Setup 64-bit developer environment
call "%VS_DIR%\VC\Auxiliary\Build\vcvars64.bat" >nul

:: Locate CMake
for /f "usebackq tokens=*" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -find "**/CMake/bin/cmake.exe"`) do (
    set "CMAKE_EXE=%%i"
)

if not defined CMAKE_EXE (
    set "CMAKE_EXE=cmake"
)

echo [*] Using CMake: %CMAKE_EXE%

if not exist build (
    mkdir build
)

cd build

echo [*] Configuring CMake project (Release, Static CRT /MT)...
"%CMAKE_EXE%" .. -G "Visual Studio 18 2026" -A x64 -DCMAKE_BUILD_TYPE=Release
if %ERRORLEVEL% neq 0 (
    echo [!] Falling back to standard generator...
    "%CMAKE_EXE%" .. -A x64 -DCMAKE_BUILD_TYPE=Release
)

echo [*] Compiling ORLogin...
"%CMAKE_EXE%" --build . --config Release --target ORLogin
set BUILD_RESULT=%ERRORLEVEL%

if %BUILD_RESULT% equ 0 (
    echo.
    echo =========================================================
    echo   BUILD SUCCESSFUL!
    echo =========================================================
    if exist Release\ORLogin.exe (
        dir Release\ORLogin.exe
    )
) else (
    echo.
    echo [ERROR] Build failed with error code %BUILD_RESULT%.
)

cd ..
endlocal & exit /b %BUILD_RESULT%
