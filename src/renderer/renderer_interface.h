#pragma once

#include <windows.h>

class IRenderer {
public:
    virtual ~IRenderer() = default;

    // Initializes the Direct3D device & ImGui backend for HWND
    virtual bool Init(HWND hwnd) = 0;

    // Releases device & backbuffers
    virtual void Cleanup() = 0;

    // Handles window resize event
    virtual void Resize(UINT width, UINT height) = 0;

    // Begins a new rendering frame
    virtual void BeginFrame() = 0;

    // Ends rendering frame and presents backbuffer
    virtual void EndFrame(const float clear_color[4]) = 0;

    // Human-readable backend name for logging/debugging
    virtual const char* GetName() const = 0;
};
