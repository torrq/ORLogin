#pragma once

// Global hotkey registration and formatting helpers.

#include <windows.h>
#include <string>

namespace Hotkey {
    constexpr int kHotkeyId = 1001;

    // Registers or re-registers the global hotkey on `hwnd`.
    bool Register(HWND hwnd, UINT mods, UINT vk);

    // Unregisters the global hotkey from `hwnd`.
    void Unregister(HWND hwnd);

    // Human-readable label (e.g. "Ctrl + Shift + L").
    std::string Format(UINT mods, UINT vk);

    // Name of a single virtual key (e.g. "F1", "L", "Insert").
    std::string KeyName(UINT vk);

    // Returns true if `vk` is a modifier key itself (Ctrl, Shift, Alt, Win).
    bool IsModifierKey(UINT vk);
}
