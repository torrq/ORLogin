#include "game/process_reader.h"

#include <vector>

bool ProcessReader::Open(DWORD pid) {
    Close();
    // Same access strategy as ORTools: try the richer set first, fall back to VM_READ only.
    m_handle = OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, FALSE, pid);
    m_canSync = m_handle != nullptr;
    if (!m_handle) m_handle = OpenProcess(PROCESS_VM_READ, FALSE, pid);
    if (!m_handle) return false;
    m_pid = pid;
    return true;
}

void ProcessReader::Close() {
    if (m_handle) CloseHandle(m_handle);
    m_handle  = nullptr;
    m_pid     = 0;
    m_canSync = false;
}

bool ProcessReader::HasExited() const {
    if (!m_handle) return true;
    if (!m_canSync) return false;
    return WaitForSingleObject(m_handle, 0) == WAIT_OBJECT_0;
}

bool ProcessReader::Read(uint32_t address, void* out, size_t size) const {
    if (!m_handle || address == 0) return false;
    SIZE_T read = 0;
    BOOL ok = ReadProcessMemory(m_handle, reinterpret_cast<LPCVOID>(static_cast<uintptr_t>(address)),
                                out, size, &read);
    return ok && read == size;
}

std::string ProcessReader::ReadString(uint32_t address, size_t maxLen) const {
    std::vector<char> buf(maxLen + 1, 0);
    if (!Read(address, buf.data(), maxLen)) return {};
    size_t len = strnlen(buf.data(), maxLen);
    if (len == 0) return {};

    // First attempt Windows-1252 (Western European / Ragnarok Online English client encoding)
    int wlen = MultiByteToWideChar(1252, 0, buf.data(), (int)len, nullptr, 0);
    if (wlen <= 0) {
        // Fallback to system ANSI code page
        wlen = MultiByteToWideChar(CP_ACP, 0, buf.data(), (int)len, nullptr, 0);
    }
    if (wlen <= 0) return {};

    std::wstring w(wlen, 0);
    if (MultiByteToWideChar(1252, 0, buf.data(), (int)len, &w[0], wlen) <= 0) {
        MultiByteToWideChar(CP_ACP, 0, buf.data(), (int)len, &w[0], wlen);
    }

    int ulen = WideCharToMultiByte(CP_UTF8, 0, w.data(), wlen, nullptr, 0, nullptr, nullptr);
    if (ulen <= 0) return {};

    std::string out(ulen, 0);
    WideCharToMultiByte(CP_UTF8, 0, w.data(), wlen, &out[0], ulen, nullptr, nullptr);
    return out;
}
