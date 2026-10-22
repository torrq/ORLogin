#include "settings.h"

#include <algorithm>

#include "fs_utils.h"
#include "vendor/json.hpp"

using json = nlohmann::json;

namespace SettingsIO {

Settings Load(const std::filesystem::path& path) {
    Settings s;
    std::string text;
    if (!FsUtils::ReadTextFile(path, text)) return s;

    json j = json::parse(text, nullptr, /*allow_exceptions*/ false);
    if (!j.is_object()) return s;

    s.darkTheme          = j.value("theme", std::string("dark")) != "light";
    s.hotkeyVk           = j.value("hotkeyVk", s.hotkeyVk);
    s.hotkeyMods         = j.value("hotkeyMods", s.hotkeyMods);
    s.sequence           = j.value("sequence", 0) == 1 ? LoginSequence::PasswordOnly : LoginSequence::IdTabPassword;
    s.clearIdField       = j.value("clearIdField", s.clearIdField);
    s.pressEnter         = j.value("pressEnter", s.pressEnter);
    s.inputMethod        = j.value("inputMethod", 0) == 1 ? InputMethod::SendInput : InputMethod::PostMessage;
    s.keyDelayMs         = std::clamp(j.value("keyDelayMs", s.keyDelayMs), 1, 200);
    s.autoLockMinutes    = std::clamp(j.value("autoLockMinutes", s.autoLockMinutes), 0, 240);
    s.requireLoginScreen = j.value("requireLoginScreen", s.requireLoginScreen);
    s.hideFromCapture    = j.value("hideFromCapture", s.hideFromCapture);

    s.hotkeyMods &= (MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN);
    if (s.hotkeyVk == 0 || s.hotkeyVk > 0xFE) s.hotkeyVk = 'L';
    return s;
}

bool Save(const std::filesystem::path& path, const Settings& s) {
    json j;
    j["theme"]              = s.darkTheme ? "dark" : "light";
    j["hotkeyVk"]           = s.hotkeyVk;
    j["hotkeyMods"]         = s.hotkeyMods;
    j["sequence"]           = (int)s.sequence;
    j["clearIdField"]       = s.clearIdField;
    j["pressEnter"]         = s.pressEnter;
    j["inputMethod"]        = (int)s.inputMethod;
    j["keyDelayMs"]         = s.keyDelayMs;
    j["autoLockMinutes"]    = s.autoLockMinutes;
    j["requireLoginScreen"] = s.requireLoginScreen;
    j["hideFromCapture"]    = s.hideFromCapture;
    return FsUtils::WriteTextFile(path, j.dump(2));
}

} // namespace SettingsIO
