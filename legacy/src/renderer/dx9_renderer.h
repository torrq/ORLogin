#pragma once

#include "renderer_interface.h"
#include <d3d9.h>
#include <string>

class DX9Renderer : public IRenderer {
public:
    DX9Renderer();
    ~DX9Renderer() override;

    bool Init(HWND hwnd) override;
    void Cleanup() override;
    void Resize(UINT width, UINT height) override;
    void BeginFrame() override;
    void EndFrame(const float clear_color[4]) override;
    const char* GetName() const override;

private:
    void ResetDevice();

    HWND m_hwnd = nullptr;
    LPDIRECT3D9 m_pD3D = nullptr;
    LPDIRECT3DDEVICE9 m_pd3dDevice = nullptr;
    D3DPRESENT_PARAMETERS m_d3dpp;
    bool m_initialized = false;
    bool m_deviceLost = false;
};
