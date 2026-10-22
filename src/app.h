#pragma once

#include <windows.h>
#include <filesystem>
#include <string>

#include "vendor/imgui/imgui.h"
#include "game/client_monitor.h"
#include "input/typer.h"
#include "security/secure.h"
#include "security/vault.h"
#include "settings.h"

struct StatusMessage {
    std::string text;
    ImVec4      color = ImVec4(1, 1, 1, 1);
    double      expiryTime = 0.0;
};

class App {
public:
    App();
    ~App();

    void Init(HWND hwnd);
    void RenderUI(HWND hwnd);
    void OnHotkey();
    void OnSessionLock();
    bool HandleCloseRequest(HWND hwnd);

    const char* GetWindowTitle() const { return "OSRO Login Helper"; }
    bool IsDarkTheme() const { return m_settings.darkTheme; }
    void SetSmallFont(ImFont* f) { m_smallFont = f; }

    void FlashStatus(const std::string& msg, ImVec4 color);

private:
    void RenderHeader();
    void RenderCreateVaultScreen();
    void RenderUnlockScreen();
    void RenderMainScreen();
    void RenderAccountsPanel();
    void RenderClientsPanel();
    void RenderSettingsPanel();
    void RenderAccountModal();
    void RenderChangeMasterModal();
    void RenderAdvancedSettingsModal();
    void RenderStatusBar();
    bool CheckModalDismiss(bool& isOpen, bool wipeBuffers = false);

    void ApplySettingsToSystem();
    void SaveSettings();
    void LockVault(const char* reason = nullptr);
    void TouchActivity();
    void CheckAutoLock();
    void WipeFormBuffers();
    void OpenAddAccountForm();
    void OpenEditAccountForm(int index);

    static constexpr size_t kPwBufSize = 256;

    HWND                  m_hwnd = nullptr;
    std::filesystem::path m_exeDir;
    std::filesystem::path m_settingsPath;
    Settings              m_settings;
    Vault                 m_vault;
    ClientMonitor         m_monitor;
    Typer                 m_typer;

    // Unlock / create vault buffers (VirtualLock'd + wiped on lock/submit)
    SecureBytes m_pwBuf1;
    SecureBytes m_pwBuf2;
    SecureBytes m_pwBuf3;
    bool        m_focusMasterInput = true;
    int         m_failedUnlocks    = 0;
    ULONGLONG   m_cooldownUntilTick = 0;

    // Add / edit account modal form
    bool        m_accountFormOpen   = false;
    bool        m_focusAccountField = false;
    int         m_editingIndex      = -1; // -1 = adding new
    char        m_labelBuf[64]      = {};
    char        m_userBuf[64]       = {};
    SecureBytes m_accPwBuf;
    bool        m_showAccPw         = false;

    // Inline delete confirmation
    int m_confirmDeleteIndex = -1;

    // Change master password modal
    bool m_changeMasterOpen = false;

    // Advanced timing settings modal
    bool m_advancedSettingsOpen = false;

    // Fonts
    ImFont* m_smallFont = nullptr;

    // Hotkey capture state
    bool m_capturingHotkey = false;
    bool m_hotkeyRegistered = false;

    // Timing
    ULONGLONG m_lastClientRefreshTick = 0;
    ULONGLONG m_lastActivityTick      = 0;

    StatusMessage m_status;
};
