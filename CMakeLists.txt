cmake_minimum_required(VERSION 3.15)
project(ORLogin VERSION 0.5.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Static CRT linking (/MT) to ensure zero DLL runtime dependencies on Windows 7-11
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

option(ORLOGIN_PORTABLE "Default to portable storage next to executable" OFF)

set(IMGUI_DIR "${CMAKE_CURRENT_SOURCE_DIR}/src/vendor/imgui")

include_directories(
    "${CMAKE_CURRENT_SOURCE_DIR}/src"
    "${CMAKE_CURRENT_SOURCE_DIR}/src/vendor"
    "${IMGUI_DIR}"
    "${IMGUI_DIR}/backends"
)

set(APP_SOURCES
    src/main.cpp
    src/app.cpp
    src/theme.cpp
    src/settings.cpp
    src/fs_utils.cpp
    src/hotkey.cpp
    src/security/secure.cpp
    src/security/crypto.cpp
    src/security/vault.cpp
    src/game/process_reader.cpp
    src/game/client_monitor.cpp
    src/input/typer.cpp
    src/renderer/dx11_renderer.cpp
    src/renderer/dx9_renderer.cpp
    ${IMGUI_DIR}/imgui.cpp
    ${IMGUI_DIR}/imgui_draw.cpp
    ${IMGUI_DIR}/imgui_tables.cpp
    ${IMGUI_DIR}/imgui_widgets.cpp
    ${IMGUI_DIR}/backends/imgui_impl_win32.cpp
    ${IMGUI_DIR}/backends/imgui_impl_dx11.cpp
    ${IMGUI_DIR}/backends/imgui_impl_dx9.cpp
    ${IMGUI_DIR}/misc/cpp/imgui_stdlib.cpp
    ORLogin.manifest
)

set(APP_LIBS
    d3d11
    dxgi
    d3d9
    dwmapi
    shell32
    user32
    bcrypt     # CNG: PBKDF2, AES-256-GCM, RNG
    crypt32    # CryptProtectMemory
    wtsapi32   # Lock vault when the Windows session locks
)

add_executable(ORLogin WIN32 ${APP_SOURCES})
target_compile_definitions(ORLogin PRIVATE OSRO_MODE_HR UNICODE _UNICODE NOMINMAX WIN32_LEAN_AND_MEAN)
if(ORLOGIN_PORTABLE)
    target_compile_definitions(ORLogin PRIVATE ORLOGIN_PORTABLE_DEFAULT=1)
endif()

target_link_libraries(ORLogin PRIVATE ${APP_LIBS})
if(MSVC)
    target_compile_options(ORLogin PRIVATE /W3 /utf-8 /sdl /guard:cf)
    # The embedded ORLogin.manifest supplies requireAdministrator; disable the default one.
    target_link_options(ORLogin PRIVATE
        /MANIFESTUAC:NO
        /DEPENDENTLOADFLAG:0x800
        /guard:cf
        /CETCOMPAT
        /DYNAMICBASE
        /NXCOMPAT
        /HIGHENTROPYVA
    )
endif()
set_target_properties(ORLogin PROPERTIES OUTPUT_NAME "ORLogin")



