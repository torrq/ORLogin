#include "fs_utils.h"

#include <windows.h>
#include <fstream>
#include <sstream>

namespace FsUtils {

std::filesystem::path GetExeDir() {
    wchar_t buffer[MAX_PATH];
    GetModuleFileNameW(nullptr, buffer, MAX_PATH);
    std::filesystem::path exePath(buffer);
    return exePath.parent_path();
}

bool IsPortableMode() {
#ifdef ORLOGIN_PORTABLE_DEFAULT
    return true;
#else
    std::filesystem::path exeDir = GetExeDir();
    std::error_code ec;
    return std::filesystem::exists(exeDir / L"portable.flag", ec);
#endif
}

std::filesystem::path GetStorageDir() {
    std::filesystem::path exeDir = GetExeDir();
    if (IsPortableMode()) {
        return exeDir;
    }

    wchar_t localAppPath[MAX_PATH] = {};
    if (GetEnvironmentVariableW(L"LOCALAPPDATA", localAppPath, MAX_PATH) == 0) {
        // Fallback to exeDir if %LOCALAPPDATA% is unavailable
        return exeDir;
    }

    std::filesystem::path appDataDir = std::filesystem::path(localAppPath) / L"ORLogin";
    std::error_code ec;
    std::filesystem::create_directories(appDataDir, ec);

    // Automatic migration from older versions:
    // If vault exists in exeDir but not in appDataDir, copy it over
    std::filesystem::path oldVault = exeDir / L"ORLogin.vault";
    std::filesystem::path newVault = appDataDir / L"ORLogin.vault";
    if (std::filesystem::exists(oldVault, ec) && !std::filesystem::exists(newVault, ec)) {
        CopyFileW(oldVault.c_str(), newVault.c_str(), TRUE);
        std::filesystem::path oldBak = exeDir / L"ORLogin.vault.bak";
        std::filesystem::path newBak = appDataDir / L"ORLogin.vault.bak";
        if (std::filesystem::exists(oldBak, ec)) {
            CopyFileW(oldBak.c_str(), newBak.c_str(), TRUE);
        }
        std::filesystem::path oldSettings = exeDir / L"ORLogin.settings.json";
        std::filesystem::path newSettings = appDataDir / L"ORLogin.settings.json";
        if (std::filesystem::exists(oldSettings, ec) && !std::filesystem::exists(newSettings, ec)) {
            CopyFileW(oldSettings.c_str(), newSettings.c_str(), TRUE);
        }
    }

    return appDataDir;
}

bool ReadTextFile(const std::filesystem::path& path, std::string& outContent) {
    std::ifstream in(path, std::ios::binary);
    if (!in) return false;
    std::ostringstream ss;
    ss << in.rdbuf();
    outContent = ss.str();
    return true;
}

bool WriteTextFile(const std::filesystem::path& path, const std::string& content) {
    std::filesystem::path tmp = path;
    tmp += L".tmp";
    {
        std::ofstream out(tmp, std::ios::binary | std::ios::trunc);
        if (!out) return false;
        out.write(content.data(), (std::streamsize)content.size());
        if (!out) return false;
    }
    return MoveFileExW(tmp.c_str(), path.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) != 0;
}

std::wstring Widen(const std::string& s) {
    if (s.empty()) return {};
    int len = MultiByteToWideChar(CP_UTF8, 0, s.data(), (int)s.size(), nullptr, 0);
    std::wstring w(len, 0);
    MultiByteToWideChar(CP_UTF8, 0, s.data(), (int)s.size(), &w[0], len);
    return w;
}

std::string Narrow(const std::wstring& w) {
    if (w.empty()) return {};
    int len = WideCharToMultiByte(CP_UTF8, 0, w.data(), (int)w.size(), nullptr, 0, nullptr, nullptr);
    std::string s(len, 0);
    WideCharToMultiByte(CP_UTF8, 0, w.data(), (int)w.size(), &s[0], len, nullptr, nullptr);
    return s;
}

} // namespace FsUtils
