#pragma once

#include "vendor/imgui/imgui.h"

struct AppThemeColors {
    ImVec4 base;
    ImVec4 surface;
    ImVec4 panel;
    ImVec4 panelHover;
    ImVec4 border;
    ImVec4 borderBright;
    ImVec4 accent;       // highlights, check marks, active radio
    ImVec4 primary;      // primary button background
    ImVec4 primaryHover;
    ImVec4 primaryText;
    ImVec4 text;
    ImVec4 muted;
    ImVec4 green;
    ImVec4 amber;
    ImVec4 red;
};

namespace Theme {
    // Returns color tokens for the dark or light palette
    AppThemeColors GetThemeColors(bool dark);

    // Applies complete Dear ImGui styling and palette
    void ApplyTheme(bool dark);
}
