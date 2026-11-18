#pragma once

// Read-only access to another process's memory (port of ORTools ProcessMemoryReader).

#include <windows.h>
#include <cstdint>
#include <string>

class ProcessReader {
public:
    ProcessReader() = default;
    ~ProcessReader() { Close(); }

    ProcessReader(const ProcessReader&) = delete;
    ProcessReader& operator=(const ProcessReader&) = delete;

    bool  Open(DWORD pid);
    void  Close();
    bool  IsOpen() const { return m_handle != nullptr; }
    DWORD Pid() const { return m_pid; }
    bool  HasExited() const;

    bool Read(uint32_t address, void* out, size_t size) const;
    bool ReadU32(uint32_t address, uint32_t& out) const { return Read(address, &out, sizeof(out)); }

    // Reads a NUL-terminated ANSI (CP-1252) string of at most maxLen bytes, returned as UTF-8.
    std::string ReadString(uint32_t address, size_t maxLen) const;

private:
    HANDLE m_handle = nullptr;
    DWORD  m_pid = 0;
    bool   m_canSync = false;
};
