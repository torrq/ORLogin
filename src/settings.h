#pragma once

// Non-secret, plaintext user preferences. Stored as JSON next to the executable.
// NEVER put anything credential-related in here – that belongs in the Vault.

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

struct Settings {
    bool          darkTheme          = true;
    UINT          hotkeyVk           = 'L';
    UINT          hotkeyMods         = MOD_CONTROL | MOD_SHIFT;
    LoginSequence sequence           = LoginSequence::IdTabPassword;
    bool          clearIdField       = true;
    bool          pressEnter         = true;
    InputMethod   inputMethod        = InputMethod::PostMessage;
    int           keyDelayMs         = 15;
    int           autoLockMinutes    = 15;   // 0 = never
    bool          requireLoginScreen = true; // refuse to type into an in-game client
    bool          hideFromCapture    = false;// exclude ORLogin's window from screenshots/streams
};

namespace SettingsIO {
    Settings Load(const std::filesystem::path& path);
    bool     Save(const std::filesystem::path& path, const Settings& s);
}
