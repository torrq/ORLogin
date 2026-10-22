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
