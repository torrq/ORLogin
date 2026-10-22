#include "dx11_renderer.h"

#include "../vendor/imgui/imgui.h"
#include "../vendor/imgui/backends/imgui_impl_win32.h"
#include "../vendor/imgui/backends/imgui_impl_dx11.h"

#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "dxgi.lib")

DX11Renderer::DX11Renderer(D3D_DRIVER_TYPE driverType)
    : m_driverType(driverType) {
    if (driverType == D3D_DRIVER_TYPE_WARP) {
        m_name = "Direct3D 11 (WARP Software Rasterizer)";
    } else {
        m_name = "Direct3D 11 (Hardware Accelerated)";
    }
}

DX11Renderer::~DX11Renderer() {
    Cleanup();
}

void DX11Renderer::CreateRenderTarget() {
    if (!m_pSwapChain || !m_pd3dDevice) return;
    ID3D11Texture2D* pBackBuffer = nullptr;
    if (SUCCEEDED(m_pSwapChain->GetBuffer(0, IID_PPV_ARGS(&pBackBuffer)))) {
        m_pd3dDevice->CreateRenderTargetView(pBackBuffer, nullptr, &m_mainRenderTargetView);
        pBackBuffer->Release();
    }
}

void DX11Renderer::CleanupRenderTarget() {
    if (m_mainRenderTargetView) {
        m_mainRenderTargetView->Release();
        m_mainRenderTargetView = nullptr;
    }
}

bool DX11Renderer::Init(HWND hwnd) {
    m_hwnd = hwnd;

    DXGI_SWAP_CHAIN_DESC sd;
    ZeroMemory(&sd, sizeof(sd));
    sd.BufferCount = 2;
    sd.BufferDesc.Width = 0;
    sd.BufferDesc.Height = 0;
    sd.BufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    sd.BufferDesc.RefreshRate.Numerator = 60;
    sd.BufferDesc.RefreshRate.Denominator = 1;
    sd.Flags = DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH;
    sd.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    sd.OutputWindow = hwnd;
    sd.SampleDesc.Count = 1;
    sd.SampleDesc.Quality = 0;
    sd.Windowed = TRUE;
    sd.SwapEffect = DXGI_SWAP_EFFECT_DISCARD;

    UINT createDeviceFlags = 0;
    D3D_FEATURE_LEVEL featureLevel;
    const D3D_FEATURE_LEVEL featureLevelArray[2] = {
        D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_10_0,
    };

    HRESULT hr = D3D11CreateDeviceAndSwapChain(
        nullptr,
        m_driverType,
        nullptr,
        createDeviceFlags,
        featureLevelArray,
        2,
        D3D11_SDK_VERSION,
        &sd,
        &m_pSwapChain,
        &m_pd3dDevice,
        &featureLevel,
        &m_pd3dDeviceContext
    );

    if (FAILED(hr)) {
        return false;
    }

    CreateRenderTarget();

    ImGui_ImplWin32_Init(hwnd);
    ImGui_ImplDX11_Init(m_pd3dDevice, m_pd3dDeviceContext);

    m_initialized = true;
    return true;
}

void DX11Renderer::Cleanup() {
    if (!m_initialized) return;

    ImGui_ImplDX11_Shutdown();
    ImGui_ImplWin32_Shutdown();

    CleanupRenderTarget();

    if (m_pSwapChain) {
        m_pSwapChain->SetFullscreenState(FALSE, nullptr);
        m_pSwapChain->Release();
        m_pSwapChain = nullptr;
    }
    if (m_pd3dDeviceContext) {
        m_pd3dDeviceContext->Release();
        m_pd3dDeviceContext = nullptr;
    }
    if (m_pd3dDevice) {
        m_pd3dDevice->Release();
        m_pd3dDevice = nullptr;
    }

    m_initialized = false;
}

void DX11Renderer::Resize(UINT width, UINT height) {
    if (!m_pSwapChain || width == 0 || height == 0) return;

    CleanupRenderTarget();
    m_pSwapChain->ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, 0);
    CreateRenderTarget();
}

void DX11Renderer::BeginFrame() {
    ImGui_ImplDX11_NewFrame();
    ImGui_ImplWin32_NewFrame();
    ImGui::NewFrame();
}

void DX11Renderer::EndFrame(const float clear_color[4]) {
    ImGui::Render();

    if (m_mainRenderTargetView && m_pd3dDeviceContext) {
        m_pd3dDeviceContext->OMSetRenderTargets(1, &m_mainRenderTargetView, nullptr);
        m_pd3dDeviceContext->ClearRenderTargetView(m_mainRenderTargetView, clear_color);
        ImGui_ImplDX11_RenderDrawData(ImGui::GetDrawData());
    }

    if (m_pSwapChain) {
        m_pSwapChain->Present(1, 0); // VSync
    }
}

const char* DX11Renderer::GetName() const {
    return m_name.c_str();
}
