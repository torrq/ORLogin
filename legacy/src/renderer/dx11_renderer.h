#pragma once

#include "renderer_interface.h"
#include <d3d11.h>
#include <string>

class DX11Renderer : public IRenderer {
public:
    explicit DX11Renderer(D3D_DRIVER_TYPE driverType = D3D_DRIVER_TYPE_HARDWARE);
    ~DX11Renderer() override;

    bool Init(HWND hwnd) override;
    void Cleanup() override;
    void Resize(UINT width, UINT height) override;
    void BeginFrame() override;
    void EndFrame(const float clear_color[4]) override;
    const char* GetName() const override;

private:
    void CleanupRenderTarget();
    void CreateRenderTarget();

    D3D_DRIVER_TYPE m_driverType;
    std::string m_name;
    HWND m_hwnd = nullptr;

    ID3D11Device* m_pd3dDevice = nullptr;
    ID3D11DeviceContext* m_pd3dDeviceContext = nullptr;
    IDXGISwapChain* m_pSwapChain = nullptr;
    ID3D11RenderTargetView* m_mainRenderTargetView = nullptr;
    bool m_initialized = false;
};
