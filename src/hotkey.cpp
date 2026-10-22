#include "hotkey.h"

namespace Hotkey {

bool Register(HWND hwnd, UINT mods, UINT vk) {
    Unregister(hwnd);
    if (!hwnd || vk == 0) return false;
    return RegisterHotKey(hwnd, kHotkeyId, mods | MOD_NOREPEAT, vk) != 0;
}

void Unregister(HWND hwnd) {
    if (hwnd) UnregisterHotKey(hwnd, kHotkeyId);
}

bool IsModifierKey(UINT vk) {
    switch (vk) {
    case VK_SHIFT: case VK_LSHIFT: case VK_RSHIFT:
    case VK_CONTROL: case VK_LCONTROL: case VK_RCONTROL:
    case VK_MENU: case VK_LMENU: case VK_RMENU:
    case VK_LWIN: case VK_RWIN:
        return true;
    default:
        return false;
    }
}

std::string KeyName(UINT vk) {
    if (vk >= 'A' && vk <= 'Z') return std::string(1, (char)vk);
    if (vk >= '0' && vk <= '9') return std::string(1, (char)vk);
    if (vk >= VK_F1 && vk <= VK_F24) return "F" + std::to_string(vk - VK_F1 + 1);
    if (vk >= VK_NUMPAD0 && vk <= VK_NUMPAD9) return "Num" + std::to_string(vk - VK_NUMPAD0);

    switch (vk) {
    case VK_SPACE:   return "Space";
    case VK_TAB:     return "Tab";
    case VK_RETURN:  return "Enter";
    case VK_BACK:    return "Backspace";
    case VK_INSERT:  return "Insert";
    case VK_DELETE:  return "Delete";
    case VK_HOME:    return "Home";
    case VK_END:     return "End";
    case VK_PRIOR:   return "PageUp";
    case VK_NEXT:    return "PageDown";
    case VK_UP:      return "Up";
    case VK_DOWN:    return "Down";
    case VK_LEFT:    return "Left";
    case VK_RIGHT:   return "Right";
    case VK_OEM_3:   return "`";
    case VK_OEM_MINUS: return "-";
    case VK_OEM_PLUS:  return "=";
    }

    UINT scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC);
    wchar_t buf[64] = {};
    if (scan && GetKeyNameTextW((LONG)(scan << 16), buf, 63) > 0) {
        int len = WideCharToMultiByte(CP_UTF8, 0, buf, -1, nullptr, 0, nullptr, nullptr);
        if (len > 1) {
            std::string s(len - 1, 0);
            WideCharToMultiByte(CP_UTF8, 0, buf, -1, &s[0], len - 1, nullptr, nullptr);
            return s;
        }
    }
    char hex[16];
    snprintf(hex, sizeof(hex), "0x%02X", vk);
    return hex;
}

std::string Format(UINT mods, UINT vk) {
    if (vk == 0) return "None";
    std::string out;
    if (mods & MOD_CONTROL) out += "Ctrl + ";
    if (mods & MOD_SHIFT)   out += "Shift + ";
    if (mods & MOD_ALT)     out += "Alt + ";
    if (mods & MOD_WIN)     out += "Win + ";
    out += KeyName(vk);
    return out;
}

} // namespace Hotkey
