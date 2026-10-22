#include "theme.h"

namespace Theme {

static inline ImVec4 Rgb(int r, int g, int b, float a = 1.0f) {
    return ImVec4(r / 255.0f, g / 255.0f, b / 255.0f, a);
}

AppThemeColors GetThemeColors(bool dark) {
    AppThemeColors c;
    if (dark) {
        // Modern Obsidian & Cyan/Azure palette
        c.base         = Rgb(15, 17, 23);       // Deep obsidian canvas (#0F1117)
        c.surface      = Rgb(22, 27, 34);       // Elevated card surface (#161B22)
        c.panel        = Rgb(30, 36, 46);       // Controls, inputs & table headers (#1E242E)
        c.panelHover   = Rgb(40, 48, 62);       // Hover highlight (#28303E)
        c.border       = Rgb(48, 54, 61);       // Subtle crisp border (#30363D)
        c.borderBright = Rgb(75, 85, 102);      // Active focus border (#4B5566)
        c.accent       = Rgb(56, 189, 248);     // Electric Sky / Cyan accent (#38BDF8)
        c.primary      = Rgb(37, 99, 235);      // Rich royal blue CTA button (#2563EB)
        c.primaryHover = Rgb(59, 130, 246);     // Vibrant blue hover (#3B82F6)
        c.primaryText  = Rgb(255, 255, 255);
        c.text         = Rgb(240, 243, 246);     // Crisp off-white text (#F0F3F6)
        c.muted        = Rgb(139, 148, 158);     // Cool slate secondary text (#8B949E)
        c.green        = Rgb(52, 211, 153);      // Emerald mint (#34D399)
        c.amber        = Rgb(251, 191, 36);      // Warm golden amber (#FBBF24)
        c.red          = Rgb(248, 113, 113);     // Coral rose (#F87171)
    } else {
        // Clean Studio Paper & Royal Blue palette
        c.base         = Rgb(246, 248, 250);    // Crisp light background (#F6F8FA)
        c.surface      = Rgb(255, 255, 255);    // Pure white cards (#FFFFFF)
        c.panel        = Rgb(240, 243, 246);    // Soft panel (#F0F3F6)
        c.panelHover   = Rgb(226, 232, 240);    // Light hover state (#E2E8F0)
        c.border       = Rgb(208, 215, 222);    // Clean border line (#D0D7DE)
        c.borderBright = Rgb(9, 105, 218);      // Focus border (#0969DA)
        c.accent       = Rgb(9, 105, 218);      // Royal blue accent (#0969DA)
        c.primary      = Rgb(9, 105, 218);      // Royal blue CTA (#0969DA)
        c.primaryHover = Rgb(3, 84, 181);       // Deep blue hover (#0354B5)
        c.primaryText  = Rgb(255, 255, 255);
        c.text         = Rgb(31, 35, 40);       // Dark slate body text (#1F2328)
        c.muted        = Rgb(101, 109, 118);    // Medium slate secondary (#656D76)
        c.green        = Rgb(26, 127, 55);      // Forest green (#1A7F37)
        c.amber        = Rgb(154, 103, 0);      // Warm honey amber (#9A6700)
        c.red          = Rgb(207, 34, 46);      // Crimson red (#CF222E)
    }
    return c;
}

void ApplyTheme(bool dark) {
    AppThemeColors c = GetThemeColors(dark);
    ImGuiStyle& style = ImGui::GetStyle();

    style.WindowRounding    = 0.0f;
    style.ChildRounding     = 6.0f;
    style.FrameRounding     = 5.0f;
    style.PopupRounding     = 6.0f;
    style.ScrollbarRounding = 4.0f;
    style.GrabRounding      = 4.0f;
    style.TabRounding       = 5.0f;

    style.WindowBorderSize  = 0.0f;
    style.ChildBorderSize   = 1.0f;
    style.PopupBorderSize   = 1.0f;
    style.FrameBorderSize   = 1.0f;
    style.TabBorderSize     = 1.0f;

    style.WindowPadding     = ImVec2(14.0f, 14.0f);
    style.FramePadding      = ImVec2(10.0f, 6.0f);
    style.ItemSpacing       = ImVec2(10.0f, 8.0f);
    style.ItemInnerSpacing  = ImVec2(6.0f, 6.0f);
    style.CellPadding       = ImVec2(8.0f, 6.0f);
    style.ScrollbarSize     = 9.0f;

    ImVec4* colors = style.Colors;
    colors[ImGuiCol_Text]                  = c.text;
    colors[ImGuiCol_TextDisabled]          = c.muted;
    colors[ImGuiCol_WindowBg]              = c.base;
    colors[ImGuiCol_ChildBg]               = c.surface;
    colors[ImGuiCol_PopupBg]               = c.surface;
    colors[ImGuiCol_Border]                = c.border;
    colors[ImGuiCol_BorderShadow]          = ImVec4(0, 0, 0, 0);
    colors[ImGuiCol_FrameBg]               = c.panel;
    colors[ImGuiCol_FrameBgHovered]        = c.panelHover;
    colors[ImGuiCol_FrameBgActive]         = c.panelHover;
    colors[ImGuiCol_TitleBg]               = c.surface;
    colors[ImGuiCol_TitleBgActive]         = c.surface;
    colors[ImGuiCol_TitleBgCollapsed]      = c.surface;
    colors[ImGuiCol_MenuBarBg]             = c.surface;
    colors[ImGuiCol_ScrollbarBg]           = c.surface;
    colors[ImGuiCol_ScrollbarGrab]         = c.border;
    colors[ImGuiCol_ScrollbarGrabHovered]  = c.borderBright;
    colors[ImGuiCol_ScrollbarGrabActive]   = c.accent;
    colors[ImGuiCol_CheckMark]             = c.accent;
    colors[ImGuiCol_SliderGrab]            = c.accent;
    colors[ImGuiCol_SliderGrabActive]      = c.primaryHover;
    colors[ImGuiCol_Button]                = c.panel;
    colors[ImGuiCol_ButtonHovered]         = c.panelHover;
    colors[ImGuiCol_ButtonActive]          = c.borderBright;
    colors[ImGuiCol_Header]                = c.panel;
    colors[ImGuiCol_HeaderHovered]         = c.panelHover;
    colors[ImGuiCol_HeaderActive]          = c.borderBright;
    colors[ImGuiCol_Separator]             = c.border;
    colors[ImGuiCol_SeparatorHovered]      = c.borderBright;
    colors[ImGuiCol_SeparatorActive]       = c.accent;
    colors[ImGuiCol_TableHeaderBg]         = c.panel;
    colors[ImGuiCol_TableBorderStrong]     = c.border;
    colors[ImGuiCol_TableBorderLight]      = ImVec4(c.border.x, c.border.y, c.border.z, 0.40f);
    colors[ImGuiCol_TableRowBg]            = ImVec4(0, 0, 0, 0);
    colors[ImGuiCol_TableRowBgAlt]         = ImVec4(c.panel.x, c.panel.y, c.panel.z, 0.35f);
    colors[ImGuiCol_TextSelectedBg]        = ImVec4(c.accent.x, c.accent.y, c.accent.z, 0.30f);
    colors[ImGuiCol_NavHighlight]          = c.accent;
    colors[ImGuiCol_ResizeGrip]            = ImVec4(0, 0, 0, 0);
    colors[ImGuiCol_ResizeGripHovered]     = ImVec4(0, 0, 0, 0);
    colors[ImGuiCol_ResizeGripActive]      = ImVec4(0, 0, 0, 0);
    colors[ImGuiCol_ModalWindowDimBg]      = ImVec4(c.base.x, c.base.y, c.base.z, 0.85f);
}

} // namespace Theme
