#pragma once

#include <filesystem>
#include <string>

namespace FsUtils {
    // Returns directory containing the current executable
    std::filesystem::path GetExeDir();

    // Returns whether ORLogin is operating in portable mode
    bool IsPortableMode();

    // Returns the active storage directory for vault and settings:
    // - If portable mode (portable.flag present or ORLOGIN_PORTABLE_DEFAULT), returns GetExeDir()
    // - Otherwise returns %LOCALAPPDATA%\ORLogin\ (creating it if needed)
    //   and automatically migrates existing ORLogin.vault and settings from GetExeDir() if present.
    std::filesystem::path GetStorageDir();

    // Reads entire file content as string
    bool ReadTextFile(const std::filesystem::path& path, std::string& outContent);

    // Writes string content to file (UTF-8, via temp file + atomic replace)
    bool WriteTextFile(const std::filesystem::path& path, const std::string& content);

    // UTF-8 <-> UTF-16 helpers
    std::wstring Widen(const std::string& s);
    std::string  Narrow(const std::wstring& s);
}
