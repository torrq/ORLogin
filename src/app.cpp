#include "app.h"

#include <dwmapi.h>
#include <algorithm>
#include <cstdio>
#include <cstring>

#include "fs_utils.h"
#include "hotkey.h"
#include "theme.h"

#ifndef DWMWA_USE_IMMERSIVE_DARK_MODE
#define DWMWA_USE_IMMERSIVE_DARK_MODE 20
#endif
#ifndef WDA_NONE
#define WDA_NONE 0x00000000
#endif
#ifndef WDA_EXCLUDEFROMCAPTURE
#define WDA_EXCLUDEFROMCAPTURE 0x00000011
#endif

namespace {

// Estimate password strength in [0.0, 1.0] without inspecting or copying into std::string.
float EstimateStrength(const SecureBytes& buf, const char*& outLabel) {
    size_t len = SecureStrLen(buf);
    bool lower = false, upper = false, digit = false, symbol = false;
    for (size_t i = 0; i < len; ++i) {
        uint8_t c = buf[i];
        if (c >= 'a' && c <= 'z')      lower = true;
        else if (c >= 'A' && c <= 'Z') upper = true;
        else if (c >= '0' && c <= '9') digit = true;
        else                           symbol = true;
    }
    int classes = (int)lower + (int)upper + (int)digit + (int)symbol;
    float score = std::min(1.0f, (float)len / 16.0f) * 0.65f + (classes / 4.0f) * 0.35f;
    if (len < 8)       { score = std::min(score, 0.25f); outLabel = "Weak (under 8 chars)"; }
    else if (score < 0.55f) outLabel = "Fair";
    else if (score < 0.80f) outLabel = "Good";
    else                    outLabel = "Strong";
    return score;
}

bool PrimaryButton(const char* label, const AppThemeColors& c, ImVec2 size = ImVec2(0, 0)) {
    ImGui::PushStyleColor(ImGuiCol_Button, c.primary);
    ImGui::PushStyleColor(ImGuiCol_ButtonHovered, c.primaryHover);
    ImGui::PushStyleColor(ImGuiCol_ButtonActive, c.accent);
    ImGui::PushStyleColor(ImGuiCol_Text, c.primaryText);
    bool pressed = ImGui::Button(label, size);
    ImGui::PopStyleColor(4);
    return pressed;
}

bool RenderLockIconButton(const AppThemeColors& c) {
    ImVec2 size(32.0f, 26.0f);
    ImGui::PushStyleColor(ImGuiCol_Button, c.panel);
    ImGui::PushStyleColor(ImGuiCol_ButtonHovered, c.panelHover);
    ImGui::PushStyleColor(ImGuiCol_ButtonActive, c.borderBright);
    bool clicked = ImGui::Button("##lock_icon_btn", size);
    ImGui::PopStyleColor(3);

    if (ImGui::IsItemHovered()) {
        ImGui::SetTooltip("Lock Vault");
    }

    ImDrawList* draw = ImGui::GetWindowDrawList();
    ImVec2 min = ImGui::GetItemRectMin();
    ImVec2 max = ImGui::GetItemRectMax();
    ImVec2 center((min.x + max.x) * 0.5f, (min.y + max.y) * 0.5f);

    bool hovered = ImGui::IsItemHovered();
    ImU32 iconCol = ImGui::ColorConvertFloat4ToU32(hovered ? c.accent : c.text);

    // Padlock body (rounded rectangle)
    float bodyW = 12.0f;
    float bodyH = 9.0f;
    ImVec2 bodyMin(center.x - bodyW * 0.5f, center.y - 0.5f);
    ImVec2 bodyMax(center.x + bodyW * 0.5f, center.y - 0.5f + bodyH);
    draw->AddRectFilled(bodyMin, bodyMax, iconCol, 2.0f);

    // Padlock shackle (curved arch at top)
    float shackleR = 4.2f;
    ImVec2 shackleCenter(center.x, bodyMin.y);
    draw->PathArcTo(shackleCenter, shackleR, 3.14159265f, 0.0f, 12);
    draw->PathStroke(iconCol, 0, 1.8f);

    // Keyhole in the lock body
    ImU32 bgCol = ImGui::ColorConvertFloat4ToU32(hovered ? c.panelHover : c.panel);
    draw->AddCircleFilled(ImVec2(center.x, center.y + 2.5f), 1.2f, bgCol);
    draw->AddLine(ImVec2(center.x, center.y + 2.5f), ImVec2(center.x, center.y + 5.5f), bgCol, 1.2f);

    return clicked;
}

void ApplyTitleBarDarkMode(HWND hwnd, bool dark) {
    if (!hwnd) return;
    BOOL useDark = dark ? TRUE : FALSE;
    DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &useDark, sizeof(useDark));
}

} // namespace

App::App()
    : m_exeDir(FsUtils::GetExeDir())
    , m_settingsPath(m_exeDir / L"ORLogin.settings.json")
    , m_settings(SettingsIO::Load(m_settingsPath))
    , m_vault(m_exeDir / L"ORLogin.vault")
    , m_monitor(Servers::HighRate)
    , m_pwBuf1(kPwBufSize, 0)
    , m_pwBuf2(kPwBufSize, 0)
    , m_pwBuf3(kPwBufSize, 0)
    , m_accPwBuf(kPwBufSize, 0) {
}

App::~App() {
    WipeFormBuffers();
    m_vault.Lock();
}

void App::Init(HWND hwnd) {
    m_hwnd = hwnd;
    m_lastActivityTick = GetTickCount64();
    ApplySettingsToSystem();
    m_monitor.Refresh();
    m_lastClientRefreshTick = GetTickCount64();
}

bool App::HandleCloseRequest(HWND hwnd) {
    Hotkey::Unregister(hwnd);
    WipeFormBuffers();
    m_vault.Lock();
    return true;
}

void App::OnSessionLock() {
    if (m_vault.IsUnlocked()) {
        LockVault("Vault locked (Windows session locked)");
    }
}

void App::FlashStatus(const std::string& msg, ImVec4 color) {
    m_status.text       = msg;
    m_status.color      = color;
    m_status.expiryTime = ImGui::GetTime() + 4.0;
}

void App::ApplySettingsToSystem() {
    Theme::ApplyTheme(m_settings.darkTheme);
    ApplyTitleBarDarkMode(m_hwnd, m_settings.darkTheme);
    if (m_hwnd) {
        SetWindowDisplayAffinity(m_hwnd, m_settings.hideFromCapture ? WDA_EXCLUDEFROMCAPTURE : WDA_NONE);
        m_hotkeyRegistered = Hotkey::Register(m_hwnd, m_settings.hotkeyMods, m_settings.hotkeyVk);
    }
}

void App::SaveSettings() {
    SettingsIO::Save(m_settingsPath, m_settings);
}

void App::WipeFormBuffers() {
    SecureWipe(m_pwBuf1);
    SecureWipe(m_pwBuf2);
    SecureWipe(m_pwBuf3);
    SecureWipe(m_accPwBuf);
    SecureZeroMemory(m_labelBuf, sizeof(m_labelBuf));
    SecureZeroMemory(m_userBuf, sizeof(m_userBuf));
    m_showAccPw = false;
    Secure::WipeImGuiTextState();
}

void App::LockVault(const char* reason) {
    m_accountFormOpen     = false;
    m_editingIndex        = -1;
    m_confirmDeleteIndex  = -1;
    m_changeMasterOpen    = false;
    m_capturingHotkey     = false;
    m_focusMasterInput    = true;
    WipeFormBuffers();
    m_vault.Lock();
    if (reason) {
        AppThemeColors c = Theme::GetThemeColors(m_settings.darkTheme);
        FlashStatus(reason, c.amber);
    }
}

void App::TouchActivity() {
    m_lastActivityTick = GetTickCount64();
}

void App::CheckAutoLock() {
    if (!m_vault.IsUnlocked() || m_settings.autoLockMinutes <= 0) return;
    ULONGLONG idleMs  = GetTickCount64() - m_lastActivityTick;
    ULONGLONG limitMs = (ULONGLONG)m_settings.autoLockMinutes * 60ULL * 1000ULL;
    if (idleMs >= limitMs) {
        LockVault("Vault auto-locked due to inactivity");
    }
}

// ───────────────────────────── Hotkey trigger ─────────────────────────────

void App::OnHotkey() {
    TouchActivity();
    AppThemeColors c = Theme::GetThemeColors(m_settings.darkTheme);

    if (!m_vault.IsUnlocked()) {
        FlashStatus("Hotkey ignored: vault is locked", c.amber);
        return;
    }
    if (m_typer.IsBusy()) return;

    m_vault.NormalizeActive();
    int activeIdx = m_vault.ActiveIndex();
    const auto& accounts = m_vault.Accounts();
    if (activeIdx < 0 || activeIdx >= (int)accounts.size()) {
        FlashStatus("Hotkey ignored: no active account", c.amber);
        return;
    }

    // Refresh client states right before checking so we have live memory data.
    m_monitor.Refresh();
    m_lastClientRefreshTick = GetTickCount64();

    HWND fg = GetForegroundWindow();
    DWORD fgPid = 0;
    if (fg) GetWindowThreadProcessId(fg, &fgPid);

    const ClientInfo* target = m_monitor.FindByPid(fgPid);

    // Convenience fallback: if the user hasn't focused the game yet, but there is
    // exactly one client sitting at the login screen and using PostMessage, target it.
    if (!target && m_settings.inputMethod == InputMethod::PostMessage) {
        const ClientInfo* singleLogin = nullptr;
        int loginCount = 0;
        for (const auto& cl : m_monitor.Clients()) {
            if (cl.state == ClientState::AtLogin && cl.hwnd) {
                singleLogin = &cl;
                ++loginCount;
            }
        }
        if (loginCount == 1) target = singleLogin;
    }

    if (!target || !target->hwnd) {
        FlashStatus("Focus a Highrate client at the login screen first", c.amber);
        return;
    }

    if (m_settings.requireLoginScreen && target->state != ClientState::AtLogin) {
        if (target->state == ClientState::InGame) {
            FlashStatus("Blocked: client is already in-game (" + target->charName + ")", c.amber);
        } else {
            FlashStatus("Blocked: cannot read client memory (is ORLogin running as Admin?)", c.red);
        }
        return;
    }

    if (m_settings.inputMethod == InputMethod::SendInput && GetForegroundWindow() != target->hwnd) {
        SetForegroundWindow(target->hwnd);
    }

    const Account& acc = accounts[activeIdx];
    TyperJob job;
    job.hwnd         = target->hwnd;
    job.username     = acc.username;
    job.password     = acc.password.Reveal();
    job.sequence     = m_settings.sequence;
    job.currentFocus = target->loginFocus;
    job.clearIdField = m_settings.clearIdField;
    job.pressEnter   = m_settings.pressEnter;
    job.method       = m_settings.inputMethod;
    job.keyDelayMs   = m_settings.keyDelayMs;

    std::string who = acc.label.empty() ? acc.username : (acc.label + " (" + acc.username + ")");
    if (m_typer.Start(std::move(job))) {
        FlashStatus("Sent login for " + who + " -> PID " + std::to_string(target->pid), c.green);
    }
}

// ───────────────────────────── Top-level render ─────────────────────────────

void App::RenderUI(HWND hwnd) {
    ImGuiIO& io = ImGui::GetIO();
    if (io.MouseClicked[0] || io.MouseClicked[1] || io.MouseWheel != 0.0f) TouchActivity();
    for (int k = ImGuiKey_NamedKey_BEGIN; k < ImGuiKey_NamedKey_END; ++k) {
        if (ImGui::IsKeyPressed((ImGuiKey)k, false)) { TouchActivity(); break; }
    }

    ULONGLONG now = GetTickCount64();
    if (now - m_lastClientRefreshTick >= 500) {
        m_monitor.Refresh();
        m_lastClientRefreshTick = now;
    }
    CheckAutoLock();

    ImGui::SetNextWindowPos(ImVec2(0, 0));
    ImGui::SetNextWindowSize(io.DisplaySize);
    ImGuiWindowFlags flags = ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoResize |
                             ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoCollapse |
                             ImGuiWindowFlags_NoBringToFrontOnFocus;

    ImGui::Begin("ORLoginRoot", nullptr, flags);

    RenderHeader();
    ImGui::Separator();
    ImGui::Spacing();

    if (!m_vault.Exists()) {
        RenderCreateVaultScreen();
    } else if (!m_vault.IsUnlocked()) {
        RenderUnlockScreen();
    } else {
        RenderMainScreen();
    }

    RenderStatusBar();
    ImGui::End();
    (void)hwnd;
}

void App::RenderHeader() {
    AppThemeColors c = Theme::GetThemeColors(m_settings.darkTheme);

    ImGui::AlignTextToFramePadding();
    ImGui::TextColored(c.accent, "OSRO Login Helper");
    ImGui::SameLine();

    // Server badge
    ImGui::PushStyleColor(ImGuiCol_Button, c.panel);
    ImGui::PushStyleColor(ImGuiCol_ButtonHovered, c.panel);
    ImGui::PushStyleColor(ImGuiCol_ButtonActive, c.panel);
    ImGui::PushStyleColor(ImGuiCol_Text, c.accent);
    ImGui::SmallButton(m_monitor.Profile().label);
    ImGui::PopStyleColor(4);

    // Right-aligned controls: [Master Password] [Lock Icon] [Theme Toggle]
    const char* themeLabel = m_settings.darkTheme ? "Light Theme" : "Dark Theme";
    float rightWidth = ImGui::CalcTextSize(themeLabel).x + ImGui::GetStyle().FramePadding.x * 2.0f;
    if (m_vault.IsUnlocked()) {
        rightWidth += 32.0f + ImGui::GetStyle().ItemSpacing.x; // Lock icon button width
        rightWidth += ImGui::CalcTextSize("Master Password").x + ImGui::GetStyle().FramePadding.x * 2.0f + ImGui::GetStyle().ItemSpacing.x;
    }

    float avail = ImGui::GetContentRegionAvail().x;
    if (avail > rightWidth) ImGui::SameLine(ImGui::GetCursorPosX() + avail - rightWidth);
    else                    ImGui::SameLine();

    if (m_vault.IsUnlocked()) {
        if (ImGui::Button("Master Password")) {
            WipeFormBuffers();
            m_changeMasterOpen = true;
            ImGui::OpenPopup("Change Master Password");
        }
        ImGui::SameLine();
        if (RenderLockIconButton(c)) {
            LockVault("Vault locked");
        }
        ImGui::SameLine();
    }

    if (ImGui::Button(themeLabel)) {
        m_settings.darkTheme = !m_settings.darkTheme;
        ApplySettingsToSystem();
        SaveSettings();
    }
}

// ───────────────────────────── Create / Unlock screens ─────────────────────────────

void App::RenderCreateVaultScreen() {
    AppThemeColors c = Theme::GetThemeColors(m_settings.darkTheme);

    float panelW = std::min(500.0f, ImGui::GetContentRegionAvail().x);
    ImGui::SetCursorPosX((ImGui::GetWindowWidth() - panelW) * 0.5f);

    ImGui::BeginChild("CreateVaultCard", ImVec2(panelW, 340.0f), true);
    ImGui::TextColored(c.accent, "Create Encrypted Vault");
    ImGui::TextWrapped("Accounts and passwords are encrypted with AES-256-GCM using a key derived from your master password (PBKDF2-HMAC-SHA256, 600,000 iterations).");
    ImGui::Spacing();
    ImGui::TextColored(c.amber, "Important: There is no password recovery. If you forget your master password, the vault cannot be decrypted.");
    ImGui::Spacing();
    ImGui::Separator();
    ImGui::Spacing();

    ImGuiInputTextFlags pwFlags = ImGuiInputTextFlags_Password | ImGuiInputTextFlags_NoUndoRedo;
    ImGui::Text("Master Password");
    ImGui::SetNextItemWidth(-1);
    if (m_focusMasterInput) { ImGui::SetKeyboardFocusHere(); m_focusMasterInput = false; }
    ImGui::InputText("##create_pw1", reinterpret_cast<char*>(m_pwBuf1.data()), m_pwBuf1.size(), pwFlags);

    const char* strengthLabel = "";
    float strength = EstimateStrength(m_pwBuf1, strengthLabel);
    ImGui::ProgressBar(strength, ImVec2(-1, 6.0f), "");
    ImGui::TextColored(c.muted, "Strength: %s", strengthLabel);

    ImGui::Spacing();
    ImGui::Text("Confirm Master Password");
    ImGui::SetNextItemWidth(-1);
    bool submit = ImGui::InputText("##create_pw2", reinterpret_cast<char*>(m_pwBuf2.data()), m_pwBuf2.size(),
                                   pwFlags | ImGuiInputTextFlags_EnterReturnsTrue);

    ImGui::Spacing();
    if (PrimaryButton("Create Vault", c, ImVec2(-1, 34.0f)) || submit) {
        SecureBytes p1 = SecureFromEditBuffer(m_pwBuf1);
        SecureBytes p2 = SecureFromEditBuffer(m_pwBuf2);

        if (p1.size() < 4) {
            FlashStatus("Master password must be at least 4 characters", c.red);
        } else if (p1 != p2) {
            FlashStatus("Passwords do not match", c.red);
        } else {
            WipeFormBuffers();
            Vault::Status st = m_vault.Create(p1);
            if (st == Vault::Status::Ok) {
                FlashStatus("Encrypted vault created", c.green);
            } else {
                FlashStatus(Vault::StatusText(st), c.red);
            }
        }
    }
    ImGui::EndChild();
}

void App::RenderUnlockScreen() {
    AppThemeColors c = Theme::GetThemeColors(m_settings.darkTheme);

    float panelW = std::min(460.0f, ImGui::GetContentRegionAvail().x);
    ImGui::SetCursorPosX((ImGui::GetWindowWidth() - panelW) * 0.5f);
    ImGui::SetCursorPosY(ImGui::GetCursorPosY() + 24.0f);

    ImGui::BeginChild("UnlockVaultCard", ImVec2(panelW, 230.0f), true);
    ImGui::TextColored(c.accent, "Vault Locked");
    ImGui::TextColored(c.muted, "Enter your master password to decrypt saved accounts.");
    ImGui::Spacing();
    ImGui::Separator();
    ImGui::Spacing();

    ULONGLONG now = GetTickCount64();
    bool coolingDown = now < m_cooldownUntilTick;

    ImGuiInputTextFlags pwFlags = ImGuiInputTextFlags_Password | ImGuiInputTextFlags_NoUndoRedo |
                                  ImGuiInputTextFlags_EnterReturnsTrue;
    ImGui::Text("Master Password");
    ImGui::SetNextItemWidth(-1);
    if (m_focusMasterInput && !coolingDown) { ImGui::SetKeyboardFocusHere(); m_focusMasterInput = false; }
    bool enterPressed = ImGui::InputText("##unlock_pw", reinterpret_cast<char*>(m_pwBuf1.data()), m_pwBuf1.size(), pwFlags);

    ImGui::Spacing();
    if (coolingDown) {
        double secs = (double)(m_cooldownUntilTick - now) / 1000.0;
        char waitLabel[64];
        snprintf(waitLabel, sizeof(waitLabel), "Please wait %.1fs...", secs);
        ImGui::BeginDisabled();
        PrimaryButton(waitLabel, c, ImVec2(-1, 34.0f));
        ImGui::EndDisabled();
    } else if (PrimaryButton("Unlock Vault", c, ImVec2(-1, 34.0f)) || enterPressed) {
        SecureBytes master = SecureFromEditBuffer(m_pwBuf1);
        WipeFormBuffers();

        Vault::Status st = m_vault.Unlock(master);
        if (st == Vault::Status::Ok) {
            m_failedUnlocks     = 0;
            m_cooldownUntilTick = 0;
            FlashStatus("Vault unlocked", c.green);
        } else {
            ++m_failedUnlocks;
            if (m_failedUnlocks >= 3) {
                ULONGLONG delaySec = std::min<ULONGLONG>(30, 1ULL << std::min(m_failedUnlocks - 2, 5));
                m_cooldownUntilTick = GetTickCount64() + delaySec * 1000ULL;
            }
            m_focusMasterInput = true;
            FlashStatus(Vault::StatusText(st), c.red);
        }
    }
    ImGui::EndChild();
}

// ───────────────────────────── Main unlocked screen ─────────────────────────────

void App::OpenAddAccountForm() {
    WipeFormBuffers();
    m_accountFormOpen    = true;
    m_editingIndex       = -1;
    m_confirmDeleteIndex = -1;
}

void App::OpenEditAccountForm(int index) {
    const auto& accounts = m_vault.Accounts();
    if (index < 0 || index >= (int)accounts.size()) return;
    WipeFormBuffers();
    m_accountFormOpen    = true;
    m_editingIndex       = index;
    m_confirmDeleteIndex = -1;

    snprintf(m_labelBuf, sizeof(m_labelBuf), "%s", accounts[index].label.c_str());
    snprintf(m_userBuf, sizeof(m_userBuf), "%s", accounts[index].username.c_str());
    SecureBytes pw = accounts[index].password.Reveal();
    size_t copyLen = std::min(pw.size(), m_accPwBuf.size() - 1);
    if (copyLen > 0) std::memcpy(m_accPwBuf.data(), pw.data(), copyLen);
    m_accPwBuf[copyLen] = 0;
}

void App::RenderMainScreen() {
    float footerReserve = 34.0f;
    float contentH = ImGui::GetContentRegionAvail().y - footerReserve;

    float accountsH = std::max(180.0f, contentH * 0.48f);
    ImGui::BeginChild("AccountsSection", ImVec2(0, accountsH), true);
    RenderAccountsPanel();
    ImGui::EndChild();

    float bottomH  = std::max(160.0f, ImGui::GetContentRegionAvail().y - footerReserve);
    float clientsW = 310.0f;

    ImGui::BeginChild("ClientsSection", ImVec2(clientsW, bottomH), true);
    RenderClientsPanel();
    ImGui::EndChild();

    ImGui::SameLine();

    ImGui::BeginChild("SettingsSection", ImVec2(0, bottomH), true);
    RenderSettingsPanel();
    ImGui::EndChild();

    RenderChangeMasterModal();
}

void App::RenderAccountsPanel() {
    AppThemeColors c = Theme::GetThemeColors(m_settings.darkTheme);
    m_vault.NormalizeActive();
    auto& accounts = m_vault.Accounts();

    ImGui::AlignTextToFramePadding();
    ImGui::TextColored(c.accent, "Saved Accounts (%zu)", accounts.size());
    ImGui::SameLine();
    ImGui::TextColored(c.muted, "- active account fires on %s",
                       Hotkey::Format(m_settings.hotkeyMods, m_settings.hotkeyVk).c_str());

    float btnW = ImGui::CalcTextSize("+ Add Account").x + ImGui::GetStyle().FramePadding.x * 2.0f;
    float avail = ImGui::GetContentRegionAvail().x;
    if (avail > btnW) ImGui::SameLine(ImGui::GetCursorPosX() + avail - btnW);
    else              ImGui::SameLine();

    if (!m_accountFormOpen && PrimaryButton("+ Add Account", c)) {
        OpenAddAccountForm();
    }

    ImGui::Separator();

    // Inline Add / Edit form
    if (m_accountFormOpen) {
        ImGui::PushStyleColor(ImGuiCol_ChildBg, c.panel);
        ImGui::BeginChild("AccountForm", ImVec2(0, 108.0f), true);
        ImGui::TextColored(c.accent, m_editingIndex < 0 ? "Add Account" : "Edit Account");

        float fieldW = (ImGui::GetContentRegionAvail().x - ImGui::GetStyle().ItemSpacing.x * 2.0f) / 3.0f;
        ImGui::SetNextItemWidth(fieldW);
        ImGui::InputTextWithHint("##acc_label", "Label (optional)", m_labelBuf, sizeof(m_labelBuf));
        ImGui::SameLine();
        ImGui::SetNextItemWidth(fieldW);
        ImGui::InputTextWithHint("##acc_user", "Username / ID", m_userBuf, sizeof(m_userBuf));
        ImGui::SameLine();
        ImGui::SetNextItemWidth(fieldW);
        ImGuiInputTextFlags pf = ImGuiInputTextFlags_NoUndoRedo |
                                 (m_showAccPw ? ImGuiInputTextFlags_None : ImGuiInputTextFlags_Password);
        ImGui::InputTextWithHint("##acc_pw", "Password", reinterpret_cast<char*>(m_accPwBuf.data()), m_accPwBuf.size(), pf);

        ImGui::Checkbox("Show password", &m_showAccPw);
        ImGui::SameLine();
        if (PrimaryButton("Save Account", c)) {
            SecureBytes pw = SecureFromEditBuffer(m_accPwBuf);
            if (std::strlen(m_userBuf) == 0) {
                FlashStatus("Username cannot be empty", c.red);
            } else if (pw.empty()) {
                FlashStatus("Password cannot be empty", c.red);
            } else {
                Account a;
                a.label    = m_labelBuf;
                a.username = m_userBuf;
                a.password.Set(pw);
                if (m_editingIndex < 0) {
                    accounts.push_back(std::move(a));
                    if (m_vault.ActiveIndex() < 0) m_vault.SetActiveIndex((int)accounts.size() - 1);
                } else if (m_editingIndex < (int)accounts.size()) {
                    accounts[m_editingIndex] = std::move(a);
                }
                m_vault.NormalizeActive();
                m_accountFormOpen = false;
                WipeFormBuffers();

                Vault::Status st = m_vault.Save();
                FlashStatus(st == Vault::Status::Ok ? "Account saved & encrypted" : Vault::StatusText(st),
                            st == Vault::Status::Ok ? c.green : c.red);
            }
        }
        ImGui::SameLine();
        if (ImGui::Button("Cancel")) {
            m_accountFormOpen = false;
            WipeFormBuffers();
        }
        ImGui::EndChild();
        ImGui::PopStyleColor();
    }

    if (accounts.empty()) {
        ImGui::Spacing();
        ImGui::TextColored(c.muted, "No accounts saved yet. Click '+ Add Account' above to add your first login.");
        return;
    }

    ImGuiTableFlags tflags = ImGuiTableFlags_RowBg | ImGuiTableFlags_BordersInnerH |
                             ImGuiTableFlags_ScrollY | ImGuiTableFlags_SizingStretchProp;
    if (ImGui::BeginTable("AccountsTable", 5, tflags)) {
        ImGui::TableSetupColumn("Active",   ImGuiTableColumnFlags_WidthFixed, 54.0f);
        ImGui::TableSetupColumn("Label",    ImGuiTableColumnFlags_WidthStretch, 1.0f);
        ImGui::TableSetupColumn("Username", ImGuiTableColumnFlags_WidthStretch, 1.1f);
        ImGui::TableSetupColumn("Password", ImGuiTableColumnFlags_WidthStretch, 1.1f);
        ImGui::TableSetupColumn("Actions",  ImGuiTableColumnFlags_WidthFixed, 220.0f);
        ImGui::TableHeadersRow();

        int activeIdx = m_vault.ActiveIndex();
        for (int i = 0; i < (int)accounts.size(); ++i) {
            ImGui::PushID(i);
            ImGui::TableNextRow();

            ImGui::TableSetColumnIndex(0);
            bool isActive = (i == activeIdx);
            if (ImGui::RadioButton("##active", isActive)) {
                m_vault.SetActiveIndex(i);
                m_vault.Save();
            }

            ImGui::TableSetColumnIndex(1);
            if (accounts[i].label.empty()) ImGui::TextColored(c.muted, "-");
            else                           ImGui::TextUnformatted(accounts[i].label.c_str());

            ImGui::TableSetColumnIndex(2);
            ImGui::TextUnformatted(accounts[i].username.c_str());

            ImGui::TableSetColumnIndex(3);
            ImGui::SmallButton("Hold to reveal");
            if (ImGui::IsItemActive()) {
                SecureBytes revealed = accounts[i].password.Reveal();
                ImGui::SameLine();
                ImGui::TextUnformatted(reinterpret_cast<const char*>(revealed.data()),
                                       reinterpret_cast<const char*>(revealed.data() + revealed.size()));
            } else {
                ImGui::SameLine();
                ImGui::TextColored(c.muted, "********");
            }

            ImGui::TableSetColumnIndex(4);
            if (m_confirmDeleteIndex == i) {
                ImGui::TextColored(c.red, "Delete?");
                ImGui::SameLine();
                if (ImGui::SmallButton("Yes")) {
                    accounts[i].password.Clear();
                    if (!accounts[i].username.empty())
                        SecureZeroMemory(&accounts[i].username[0], accounts[i].username.size());
                    accounts.erase(accounts.begin() + i);
                    if (m_vault.ActiveIndex() == i)      m_vault.SetActiveIndex(0);
                    else if (m_vault.ActiveIndex() > i)  m_vault.SetActiveIndex(m_vault.ActiveIndex() - 1);
                    m_vault.NormalizeActive();
                    m_confirmDeleteIndex = -1;
                    m_vault.Save();
                    FlashStatus("Account deleted", c.amber);
                    ImGui::PopID();
                    break;
                }
                ImGui::SameLine();
                if (ImGui::SmallButton("No")) m_confirmDeleteIndex = -1;
            } else {
                if (ImGui::SmallButton("Send")) {
                    m_vault.SetActiveIndex(i);
                    m_vault.Save();
                    OnHotkey();
                }
                ImGui::SameLine();
                if (ImGui::SmallButton("Edit")) OpenEditAccountForm(i);
                ImGui::SameLine();
                if (ImGui::SmallButton("Delete")) m_confirmDeleteIndex = i;
            }
            ImGui::PopID();
        }
        ImGui::EndTable();
    }
}

void App::RenderClientsPanel() {
    AppThemeColors c = Theme::GetThemeColors(m_settings.darkTheme);
    const auto& clients = m_monitor.Clients();

    ImGui::AlignTextToFramePadding();
    ImGui::TextColored(c.accent, "Running Clients (%zu)", clients.size());
    ImGui::SameLine();
    if (ImGui::SmallButton("Refresh")) {
        m_monitor.Refresh();
        m_lastClientRefreshTick = GetTickCount64();
    }
    ImGui::Separator();

    if (clients.empty()) {
        ImGui::TextColored(c.muted, "No OSRO client processes detected.");
        return;
    }

    HWND fg = GetForegroundWindow();
    for (const auto& cl : clients) {
        bool focused = (cl.hwnd && cl.hwnd == fg);
        ImVec4 dotColor = c.red;
        std::string status = ClientStateText(cl.state);
        if (cl.state == ClientState::AtLogin) {
            dotColor = c.green;
            if (cl.loginFocus == LoginFieldFocus::UserId) {
                status = "At Login (User ID)";
            } else if (cl.loginFocus == LoginFieldFocus::Password) {
                status = "At Login (Password)";
            } else {
                status = "At Login";
            }
        } else if (cl.state == ClientState::InGame) {
            dotColor = c.amber;
            if (!cl.charName.empty()) {
                status = "In game: " + cl.charName;
                if (!cl.map.empty()) status += " @ " + cl.map;
            }
        }

        ImGui::TextColored(dotColor, "\xE2\x97\x8F");
        ImGui::SameLine();
        ImGui::Text("PID %lu%s", (unsigned long)cl.pid, focused ? " [Focused]" : "");
        ImGui::SameLine();
        ImGui::TextColored(dotColor, "%s", status.c_str());
    }
}

void App::RenderSettingsPanel() {
    AppThemeColors c = Theme::GetThemeColors(m_settings.darkTheme);

    ImGui::TextColored(c.accent, "Hotkey & Login Behavior");
    ImGui::Separator();

    if (ImGui::BeginTable("SettingsLayoutGrid", 2, ImGuiTableFlags_None)) {
        ImGui::TableSetupColumn("LeftOptions",  ImGuiTableColumnFlags_WidthStretch, 0.52f);
        ImGui::TableSetupColumn("RightOptions", ImGuiTableColumnFlags_WidthStretch, 0.48f);

        ImGui::TableNextRow();

        // ────── Left Column: Hotkey & Sequence ──────
        ImGui::TableSetColumnIndex(0);

        ImGui::AlignTextToFramePadding();
        ImGui::Text("Login Hotkey:");
        ImGui::SameLine();
        std::string hkLabel = m_capturingHotkey
            ? "Press keys... (Esc)"
            : Hotkey::Format(m_settings.hotkeyMods, m_settings.hotkeyVk);
        if (ImGui::Button(hkLabel.c_str())) {
            m_capturingHotkey = true;
            Hotkey::Unregister(m_hwnd);
        }
        if (!m_hotkeyRegistered && !m_capturingHotkey) {
            ImGui::TextColored(c.red, "Hotkey conflict (in use)");
        }

        ImGui::Spacing();
        ImGui::TextColored(c.muted, "Login Sequence:");
        int seq = (int)m_settings.sequence;
        if (ImGui::RadioButton("ID + Tab + Password", &seq, 0)) {
            m_settings.sequence = LoginSequence::IdTabPassword;
            SaveSettings();
        }
        if (ImGui::RadioButton("Password only (Save ID)", &seq, 1)) {
            m_settings.sequence = LoginSequence::PasswordOnly;
            SaveSettings();
        }

        if (m_settings.sequence == LoginSequence::IdTabPassword) {
            if (ImGui::Checkbox("Clear ID field first", &m_settings.clearIdField)) SaveSettings();
        }
        if (ImGui::Checkbox("Press Enter twice after typing", &m_settings.pressEnter)) SaveSettings();

        // ────── Right Column: Method, Safety & Timers ──────
        ImGui::TableSetColumnIndex(1);

        ImGui::AlignTextToFramePadding();
        ImGui::Text("Method:");
        ImGui::SameLine();
        int method = (int)m_settings.inputMethod;
        if (ImGui::RadioButton("PostMsg", &method, 0)) {
            m_settings.inputMethod = InputMethod::PostMessage;
            SaveSettings();
        }
        ImGui::SameLine();
        if (ImGui::RadioButton("SendInput", &method, 1)) {
            m_settings.inputMethod = InputMethod::SendInput;
            SaveSettings();
        }

        if (ImGui::Checkbox("Only send at Login screen", &m_settings.requireLoginScreen)) {
            SaveSettings();
        }

        if (ImGui::Checkbox("Hide from screen capture", &m_settings.hideFromCapture)) {
            ApplySettingsToSystem();
            SaveSettings();
        }

        ImGui::Spacing();
        ImGui::SetNextItemWidth(100.0f);
        if (ImGui::SliderInt("Key delay (ms)", &m_settings.keyDelayMs, 5, 60)) SaveSettings();

        ImGui::SetNextItemWidth(100.0f);
        if (ImGui::SliderInt("Auto-lock (min)", &m_settings.autoLockMinutes, 0, 60)) SaveSettings();

        ImGui::EndTable();
    }

    if (m_capturingHotkey) {
        if (GetAsyncKeyState(VK_ESCAPE) & 0x8000) {
            m_capturingHotkey  = false;
            m_hotkeyRegistered = Hotkey::Register(m_hwnd, m_settings.hotkeyMods, m_settings.hotkeyVk);
        } else {
            UINT mods = 0;
            if (GetAsyncKeyState(VK_CONTROL) & 0x8000) mods |= MOD_CONTROL;
            if (GetAsyncKeyState(VK_SHIFT)   & 0x8000) mods |= MOD_SHIFT;
            if (GetAsyncKeyState(VK_MENU)    & 0x8000) mods |= MOD_ALT;
            if ((GetAsyncKeyState(VK_LWIN) | GetAsyncKeyState(VK_RWIN)) & 0x8000) mods |= MOD_WIN;

            for (UINT vk = 0x08; vk <= 0xFE; ++vk) {
                if (vk == VK_LBUTTON || vk == VK_RBUTTON || vk == VK_MBUTTON ||
                    vk == VK_XBUTTON1 || vk == VK_XBUTTON2 || vk == VK_ESCAPE ||
                    Hotkey::IsModifierKey(vk)) continue;
                if (GetAsyncKeyState((int)vk) & 0x8000) {
                    m_settings.hotkeyMods = mods;
                    m_settings.hotkeyVk   = vk;
                    m_capturingHotkey     = false;
                    m_hotkeyRegistered    = Hotkey::Register(m_hwnd, m_settings.hotkeyMods, m_settings.hotkeyVk);
                    SaveSettings();
                    break;
                }
            }
        }
    }
}

void App::RenderChangeMasterModal() {
    if (!m_changeMasterOpen) return;
    AppThemeColors c = Theme::GetThemeColors(m_settings.darkTheme);

    ImGui::SetNextWindowSize(ImVec2(400, 260), ImGuiCond_Always);
    if (ImGui::BeginPopupModal("Change Master Password", &m_changeMasterOpen, ImGuiWindowFlags_NoResize)) {
        ImGuiInputTextFlags pf = ImGuiInputTextFlags_Password | ImGuiInputTextFlags_NoUndoRedo;

        ImGui::Text("Current Master Password");
        ImGui::SetNextItemWidth(-1);
        ImGui::InputText("##cur_mp", reinterpret_cast<char*>(m_pwBuf1.data()), m_pwBuf1.size(), pf);

        ImGui::Text("New Master Password");
        ImGui::SetNextItemWidth(-1);
        ImGui::InputText("##new_mp1", reinterpret_cast<char*>(m_pwBuf2.data()), m_pwBuf2.size(), pf);

        ImGui::Text("Confirm New Master Password");
        ImGui::SetNextItemWidth(-1);
        ImGui::InputText("##new_mp2", reinterpret_cast<char*>(m_pwBuf3.data()), m_pwBuf3.size(), pf);

        ImGui::Spacing();
        if (PrimaryButton("Update Master Password", c)) {
            SecureBytes cur = SecureFromEditBuffer(m_pwBuf1);
            SecureBytes n1  = SecureFromEditBuffer(m_pwBuf2);
            SecureBytes n2  = SecureFromEditBuffer(m_pwBuf3);
            if (n1.size() < 4) {
                FlashStatus("New master password must be at least 4 characters", c.red);
            } else if (n1 != n2) {
                FlashStatus("New passwords do not match", c.red);
            } else {
                WipeFormBuffers();
                Vault::Status st = m_vault.ChangeMasterPassword(cur, n1);
                if (st == Vault::Status::Ok) {
                    m_changeMasterOpen = false;
                    ImGui::CloseCurrentPopup();
                    FlashStatus("Master password updated & vault re-encrypted", c.green);
                } else {
                    FlashStatus(Vault::StatusText(st), c.red);
                }
            }
        }
        ImGui::SameLine();
        if (ImGui::Button("Cancel")) {
            WipeFormBuffers();
            m_changeMasterOpen = false;
            ImGui::CloseCurrentPopup();
        }
        ImGui::EndPopup();
    }
}

void App::RenderStatusBar() {
    AppThemeColors c = Theme::GetThemeColors(m_settings.darkTheme);
    ImGui::Separator();

    if (!m_status.text.empty() && ImGui::GetTime() < m_status.expiryTime) {
        ImGui::TextColored(m_status.color, "%s", m_status.text.c_str());
    } else {
        ImGui::TextColored(c.muted, "Vault: %s  |  Hotkey: %s",
                           m_vault.IsUnlocked() ? "Unlocked (AES-256-GCM)" : "Locked",
                           Hotkey::Format(m_settings.hotkeyMods, m_settings.hotkeyVk).c_str());
    }
}
