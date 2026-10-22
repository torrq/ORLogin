#pragma once

// User preferences:
// - Cosmetic & startup preferences: stored as plaintext JSON in the storage directory.
// - Security-critical preferences: stored authenticated and encrypted inside the Vault payload (v2).
// Credentials (usernames, passwords) NEVER touch settings – they belong strictly in the Vault.

#include <windows.h>
#include <filesystem>

enum class LoginSequence : int {
    IdTabPassword = 0, // [clear ID] -> username -> Tab -> password -> [Enter]
    PasswordOnly  = 1, // password -> [Enter]   (client "Save ID" already filled the ID)
};

enum class InputMethod : int {
    PostMessage = 0, // WM_CHAR/WM_KEYDOWN posted to the client window (works unfocused)
    SendInput   = 1, // hardware-like scancodes to the foreground window
};

// Security-critical preferences stored encrypted and authenticated inside the vault payload (v2)
struct VaultSettings {
    LoginSequence sequence           = LoginSequence::IdTabPassword;
    bool          clearIdField       = true;
    bool          pressEnter         = true;
    InputMethod   inputMethod        = InputMethod::PostMessage;
    int           keyDelayMs         = 15;
    int           enterDelayMs       = 300;  // pause between 1st Return and 2nd Return (ms)
    int           tabDelayMs         = 25;   // pause after Tab key focus transition (ms)
    int           clearDelayMs       = 3;    // delay between backspaces when clearing ID field (ms)
    int           keyHoldMs          = 10;   // key down hold duration before release (ms)
    int           autoLockMinutes    = 15;   // 0 = never
    bool          requireLoginScreen = true; // refuse to type into an in-game client
};

struct Settings {
    // Cosmetic & UI preferences (stored in plain JSON)
    bool          darkTheme          = true;
    UINT          hotkeyVk           = 'L';
    UINT          hotkeyMods         = MOD_CONTROL | MOD_SHIFT;
    bool          hideFromCapture    = false;// exclude ORLogin's window from screenshots/streams (default false)

    // Security-critical preferences (synchronized with encrypted vault payload v2)
    LoginSequence sequence           = LoginSequence::IdTabPassword;
    bool          clearIdField       = true;
    bool          pressEnter         = true;
    InputMethod   inputMethod        = InputMethod::PostMessage;
    int           keyDelayMs         = 15;
    int           enterDelayMs       = 300;
    int           tabDelayMs         = 25;
    int           clearDelayMs       = 3;
    int           keyHoldMs          = 10;
    int           autoLockMinutes    = 15;
    bool          requireLoginScreen = true;

    VaultSettings ToVaultSettings() const {
        VaultSettings vs;
        vs.sequence           = sequence;
        vs.clearIdField       = clearIdField;
        vs.pressEnter         = pressEnter;
        vs.inputMethod        = inputMethod;
        vs.keyDelayMs         = keyDelayMs;
        vs.enterDelayMs       = enterDelayMs;
        vs.tabDelayMs         = tabDelayMs;
        vs.clearDelayMs       = clearDelayMs;
        vs.keyHoldMs          = keyHoldMs;
        vs.autoLockMinutes    = autoLockMinutes;
        vs.requireLoginScreen = requireLoginScreen;
        return vs;
    }

    void FromVaultSettings(const VaultSettings& vs) {
        sequence           = vs.sequence;
        clearIdField       = vs.clearIdField;
        pressEnter         = vs.pressEnter;
        inputMethod        = vs.inputMethod;
        keyDelayMs         = vs.keyDelayMs;
        enterDelayMs       = vs.enterDelayMs;
        tabDelayMs         = vs.tabDelayMs;
        clearDelayMs       = vs.clearDelayMs;
        keyHoldMs          = vs.keyHoldMs;
        autoLockMinutes    = vs.autoLockMinutes;
        requireLoginScreen = vs.requireLoginScreen;
    }
};

namespace SettingsIO {
    Settings Load(const std::filesystem::path& path);
    bool     Save(const std::filesystem::path& path, const Settings& s);
}
