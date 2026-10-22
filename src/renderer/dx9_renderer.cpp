#include "dx9_renderer.h"

#include "../vendor/imgui/imgui.h"
#include "../vendor/imgui/backends/imgui_impl_win32.h"
#include "../vendor/imgui/backends/imgui_impl_dx9.h"

#pragma comment(lib, "d3d9.lib")

DX9Renderer::DX9Renderer() {
    ZeroMemory(&m_d3dpp, sizeof(m_d3dpp));
}

DX9Renderer::~DX9Renderer() {
    Cleanup();
}

bool DX9Renderer::Init(HWND hwnd) {
    m_hwnd = hwnd;

    m_pD3D = Direct3DCreate9(D3D_SDK_VERSION);
    if (!m_pD3D) {
        return false;
    }

    ZeroMemory(&m_d3dpp, sizeof(m_d3dpp));
    m_d3dpp.Windowed = TRUE;
    m_d3dpp.SwapEffect = D3DSWAPEFFECT_DISCARD;
    m_d3dpp.BackBufferFormat = D3DFMT_UNKNOWN;
    m_d3dpp.EnableAutoDepthStencil = FALSE;
    m_d3dpp.PresentationInterval = D3DPRESENT_INTERVAL_ONE; // VSync

    HRESULT hr = m_pD3D->CreateDevice(
        D3DADAPTER_DEFAULT,
        D3DDEVTYPE_HAL,
        hwnd,
        D3DCREATE_HARDWARE_VERTEXPROCESSING,
        &m_d3dpp,
        &m_pd3dDevice
    );

    if (FAILED(hr)) {
        // Try software vertex processing fallback
        hr = m_pD3D->CreateDevice(
            D3DADAPTER_DEFAULT,
            D3DDEVTYPE_HAL,
            hwnd,
            D3DCREATE_SOFTWARE_VERTEXPROCESSING,
            &m_d3dpp,
            &m_pd3dDevice
        );
        if (FAILED(hr)) {
            m_pD3D->Release();
            m_pD3D = nullptr;
            return false;
        }
    }

    ImGui_ImplWin32_Init(hwnd);
    ImGui_ImplDX9_Init(m_pd3dDevice);

    m_initialized = true;
    return true;
}

void DX9Renderer::ResetDevice() {
    ImGui_ImplDX9_InvalidateDeviceObjects();
    HRESULT hr = m_pd3dDevice->Reset(&m_d3dpp);
    if (hr == D3DERR_INVALIDCALL) {
        // Error handling
    }
    ImGui_ImplDX9_CreateDeviceObjects();
}

void DX9Renderer::Cleanup() {
    if (!m_initialized) return;

    ImGui_ImplDX9_Shutdown();
    ImGui_ImplWin32_Shutdown();

    if (m_pd3dDevice) {
        m_pd3dDevice->Release();
        m_pd3dDevice = nullptr;
    }
    if (m_pD3D) {
        m_pD3D->Release();
        m_pD3D = nullptr;
    }

    m_initialized = false;
}

void DX9Renderer::Resize(UINT width, UINT height) {
    if (!m_pd3dDevice || width == 0 || height == 0) return;

    m_d3dpp.BackBufferWidth = width;
    m_d3dpp.BackBufferHeight = height;
    ResetDevice();
}

void DX9Renderer::BeginFrame() {
    if (m_deviceLost) {
        HRESULT hr = m_pd3dDevice->TestCooperativeLevel();
        if (hr == D3DERR_DEVICELOST) {
            Sleep(10);
            return;
        }
        if (hr == D3DERR_DEVICENOTRESET) {
            ResetDevice();
        }
        m_deviceLost = false;
    }

    ImGui_ImplDX9_NewFrame();
    ImGui_ImplWin32_NewFrame();
    ImGui::NewFrame();
}

void DX9Renderer::EndFrame(const float clear_color[4]) {
    ImGui::Render();

    D3DCOLOR clearColor = D3DCOLOR_RGBA(
        (int)(clear_color[0] * 255.0f),
        (int)(clear_color[1] * 255.0f),
        (int)(clear_color[2] * 255.0f),
        (int)(clear_color[3] * 255.0f)
    );

    m_pd3dDevice->SetRenderState(D3DRS_ZENABLE, FALSE);
    m_pd3dDevice->SetRenderState(D3DRS_ALPHABLENDENABLE, FALSE);
    m_pd3dDevice->SetRenderState(D3DRS_SCISSORTESTENABLE, FALSE);

    m_pd3dDevice->Clear(0, nullptr, D3DCLEAR_TARGET, clearColor, 1.0f, 0);

    if (SUCCEEDED(m_pd3dDevice->BeginScene())) {
        ImGui_ImplDX9_RenderDrawData(ImGui::GetDrawData());
        m_pd3dDevice->EndScene();
    }

    HRESULT hr = m_pd3dDevice->Present(nullptr, nullptr, nullptr, nullptr);
    if (hr == D3DERR_DEVICELOST) {
        m_deviceLost = true;
    }
}

const char* DX9Renderer::GetName() const {
    return "Direct3D 9 (Legacy Universal)";
}
