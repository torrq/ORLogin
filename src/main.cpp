#include <windows.h>
#include <wtsapi32.h>
#include <memory>
#include <string>

#include "app.h"
#include "hotkey.h"
#include "security/secure.h"
#include "theme.h"
#include "renderer/renderer_interface.h"
#include "renderer/dx11_renderer.h"
#include "renderer/dx9_renderer.h"

#include "vendor/imgui/imgui.h"
#include "vendor/imgui/backends/imgui_impl_win32.h"

// Forward declare message handler from imgui_impl_win32.cpp
extern IMGUI_IMPL_API LRESULT ImGui_ImplWin32_WndProcHandler(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam);

static App*                        g_pApp      = nullptr;
static std::unique_ptr<IRenderer>  g_pRenderer = nullptr;

LRESULT WINAPI WndProc(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    if (ImGui_ImplWin32_WndProcHandler(hWnd, msg, wParam, lParam)) {
        return true;
    }

    switch (msg) {
    case WM_SIZE:
        if (g_pRenderer && wParam != SIZE_MINIMIZED) {
            g_pRenderer->Resize((UINT)LOWORD(lParam), (UINT)HIWORD(lParam));
        }
        return 0;

    case WM_HOTKEY:
        if (g_pApp && (int)wParam == Hotkey::kHotkeyId) {
            g_pApp->OnHotkey();
        }
        return 0;

    case WM_WTSSESSION_CHANGE:
        if (g_pApp && wParam == WTS_SESSION_LOCK) {
            g_pApp->OnSessionLock();
        }
        return 0;

    case WM_CLOSE:
        if (g_pApp && !g_pApp->HandleCloseRequest(hWnd)) {
            return 0;
        }
        WTSUnRegisterSessionNotification(hWnd);
        DestroyWindow(hWnd);
        return 0;

    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;
    }

    return DefWindowProcW(hWnd, msg, wParam, lParam);
}

int WINAPI wWinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, PWSTR pCmdLine, int nCmdShow) {
    (void)hInstance; (void)hPrevInstance; (void)pCmdLine;

    // Raise working-set limits so VirtualLock succeeds for our secret buffers.
    Secure::InitProcessMemory();

    // Enable DPI awareness for crisp text rendering on Win7-11
    ImGui_ImplWin32_EnableDpiAwareness();

    App app;
    g_pApp = &app;

    WNDCLASSEXW wc = {
        sizeof(wc),
        CS_CLASSDC,
        WndProc,
        0L,
        0L,
        GetModuleHandleW(nullptr),
        nullptr,
        LoadCursor(nullptr, IDC_ARROW),
        nullptr,
        nullptr,
        L"ORLoginMainWindowClass",
        nullptr
    };
    RegisterClassExW(&wc);

    std::string titleStr = app.GetWindowTitle();
    int len = MultiByteToWideChar(CP_UTF8, 0, titleStr.c_str(), -1, nullptr, 0);
    std::wstring wTitle(len, 0);
    MultiByteToWideChar(CP_UTF8, 0, titleStr.c_str(), -1, &wTitle[0], len);

    HWND hwnd = CreateWindowW(
        wc.lpszClassName,
        wTitle.c_str(),
        WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        880,
        720,
        nullptr,
        nullptr,
        wc.hInstance,
        nullptr
    );

    if (!hwnd) {
        UnregisterClassW(wc.lpszClassName, wc.hInstance);
        return 1;
    }

    // Lock the vault automatically when the user locks their workstation (Win+L).
    WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);

    IMGUI_CHECKVERSION();
    ImGui::CreateContext();
    ImGuiIO& io = ImGui::GetIO();
    io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard;
    io.IniFilename  = nullptr; // Do not write imgui.ini to disk

    // ───────────────── TYPOGRAPHY & EXTENDED GLYPH RANGES ─────────────────
    // Load Windows system fonts (Segoe UI / Tahoma / Arial) with glyph ranges
    // that cover Basic Latin, Latin Supplement, General Punctuation,
    // Letterlike Symbols (including U+2122 ™ for RO character names),
    // Arrows, and Geometric Shapes (bullet points ●).
    static const ImWchar glyphRanges[] = {
        0x0020, 0x00FF, // Basic Latin + Latin Supplement
        0x2000, 0x206F, // General Punctuation
        0x2100, 0x214F, // Letterlike Symbols (U+2122 ™ Trademark, etc.)
        0x2190, 0x21FF, // Arrows
        0x25A0, 0x25FF, // Geometric Shapes (● bullets, squares, etc.)
        0,
    };

    ImFontConfig fontCfg;
    fontCfg.OversampleH = 2;
    fontCfg.OversampleV = 2;
    fontCfg.PixelSnapH  = true;

    ImFont* loadedFont = nullptr;
    ImFont* smallFont  = nullptr;
    const char* fontCandidates[] = {
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\tahoma.ttf",
        "C:\\Windows\\Fonts\\arial.ttf"
    };
    for (const char* path : fontCandidates) {
        if (GetFileAttributesA(path) != INVALID_FILE_ATTRIBUTES) {
            loadedFont = io.Fonts->AddFontFromFileTTF(path, 18.0f, &fontCfg, glyphRanges);
            smallFont  = io.Fonts->AddFontFromFileTTF(path, 14.0f, &fontCfg, glyphRanges);
            if (loadedFont) break;
        }
    }
    if (!loadedFont) {
        loadedFont = io.Fonts->AddFontDefault();
    }
    app.SetSmallFont(smallFont);

    // ───────────────── CASCADING RENDERER INITIALIZATION ─────────────────
    g_pRenderer = std::make_unique<DX11Renderer>(D3D_DRIVER_TYPE_HARDWARE);
    if (!g_pRenderer->Init(hwnd)) {
        g_pRenderer->Cleanup();

        g_pRenderer = std::make_unique<DX11Renderer>(D3D_DRIVER_TYPE_WARP);
        if (!g_pRenderer->Init(hwnd)) {
            g_pRenderer->Cleanup();

            g_pRenderer = std::make_unique<DX9Renderer>();
            if (!g_pRenderer->Init(hwnd)) {
                g_pRenderer->Cleanup();
                MessageBoxW(
                    hwnd,
                    L"Failed to initialize graphics subsystem.\nNeither Direct3D 11 nor Direct3D 9 could be started.",
                    L"Fatal Initialization Error",
                    MB_ICONERROR | MB_OK
                );
                DestroyWindow(hwnd);
                UnregisterClassW(wc.lpszClassName, wc.hInstance);
                return 1;
            }
        }
    }

    app.Init(hwnd);

    ShowWindow(hwnd, nCmdShow);
    UpdateWindow(hwnd);

    // ───────────────── MAIN MESSAGE & RENDER LOOP ─────────────────
    bool done = false;
    while (!done) {
        MSG msg;
        while (PeekMessageW(&msg, nullptr, 0U, 0U, PM_REMOVE)) {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if (msg.message == WM_QUIT) {
                done = true;
            }
        }
        if (done) break;

        AppThemeColors c = Theme::GetThemeColors(app.IsDarkTheme());
        const float clearColor[4] = { c.base.x, c.base.y, c.base.z, 1.0f };

        g_pRenderer->BeginFrame();
        app.RenderUI(hwnd);
        g_pRenderer->EndFrame(clearColor);
    }

    g_pRenderer->Cleanup();
    g_pRenderer.reset();

    ImGui::DestroyContext();

    DestroyWindow(hwnd);
    UnregisterClassW(wc.lpszClassName, wc.hInstance);

    return 0;
}
