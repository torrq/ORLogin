#include "theme.h"

namespace Theme {

static inline ImVec4 Rgb(int r, int g, int b, float a = 1.0f) {
    return ImVec4(r / 255.0f, g / 255.0f, b / 255.0f, a);
}

AppThemeColors GetThemeColors(bool dark) {
    AppThemeColors c;
    if (dark) {
        // Dark: Emerald & Mint (OSRO Highrate identity, from rodie)
        c.base         = Rgb(8, 15, 12);
        c.surface      = Rgb(10, 26, 20);
        c.panel        = Rgb(12, 31, 26);
        c.panelHover   = Rgb(31, 63, 53);
        c.border       = Rgb(46, 90, 74);
        c.borderBright = Rgb(63, 122, 99);
        c.accent       = Rgb(0, 230, 145);
        c.primary      = Rgb(0, 150, 90);
        c.primaryHover = Rgb(0, 185, 112);
        c.primaryText  = Rgb(255, 255, 255);
        c.text         = Rgb(230, 240, 235);
        c.muted        = Rgb(158, 197, 184);
        c.green        = Rgb(34, 255, 170);
        c.amber        = Rgb(245, 192, 74);
        c.red          = Rgb(255, 107, 107);
    } else {
        // Light: Paper & Emerald
        c.base         = Rgb(242, 246, 244);
        c.surface      = Rgb(255, 255, 255);
        c.panel        = Rgb(234, 240, 237);
        c.panelHover   = Rgb(218, 231, 225);
        c.border       = Rgb(190, 208, 199);
        c.borderBright = Rgb(140, 175, 160);
        c.accent       = Rgb(0, 140, 88);
        c.primary      = Rgb(0, 140, 88);
        c.primaryHover = Rgb(0, 165, 104);
        c.primaryText  = Rgb(255, 255, 255);
        c.text         = Rgb(22, 34, 29);
        c.muted        = Rgb(92, 112, 103);
        c.green        = Rgb(0, 145, 85);
        c.amber        = Rgb(180, 112, 0);
        c.red          = Rgb(200, 45, 45);
    }
    return c;
}

void ApplyTheme(bool dark) {
    AppThemeColors c = GetThemeColors(dark);
    ImGuiStyle& style = ImGui::GetStyle();

    style.WindowRounding    = 0.0f;
    style.ChildRounding     = 8.0f;
    style.FrameRounding     = 6.0f;
    style.PopupRounding     = 8.0f;
    style.ScrollbarRounding = 4.0f;
    style.GrabRounding      = 4.0f;
    style.TabRounding       = 6.0f;

    style.WindowBorderSize  = 0.0f;
    style.ChildBorderSize   = 1.0f;
    style.PopupBorderSize   = 1.0f;
    style.FrameBorderSize   = 1.0f;
    style.TabBorderSize     = 1.0f;

    style.WindowPadding     = ImVec2(16.0f, 16.0f);
    style.FramePadding      = ImVec2(10.0f, 6.0f);
    style.ItemSpacing       = ImVec2(10.0f, 8.0f);
    style.ItemInnerSpacing  = ImVec2(6.0f, 6.0f);
    style.CellPadding       = ImVec2(8.0f, 6.0f);
    style.ScrollbarSize     = 8.0f;

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
    colors[ImGuiCol_TableBorderLight]      = c.border;
    colors[ImGuiCol_TableRowBg]            = ImVec4(0, 0, 0, 0);
    colors[ImGuiCol_TableRowBgAlt]         = ImVec4(c.panel.x, c.panel.y, c.panel.z, 0.45f);
    colors[ImGuiCol_TextSelectedBg]        = ImVec4(c.accent.x, c.accent.y, c.accent.z, 0.35f);
    colors[ImGuiCol_NavHighlight]          = c.accent;
    colors[ImGuiCol_ResizeGrip]            = ImVec4(0, 0, 0, 0);
    colors[ImGuiCol_ResizeGripHovered]     = ImVec4(0, 0, 0, 0);
    colors[ImGuiCol_ResizeGripActive]      = ImVec4(0, 0, 0, 0);
    colors[ImGuiCol_ModalWindowDimBg]      = ImVec4(c.base.x, c.base.y, c.base.z, 0.80f);
}

} // namespace Theme
