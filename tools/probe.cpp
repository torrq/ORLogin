#include <windows.h>
#include <tlhelp32.h>
#include <iostream>
#include <fstream>
#include <iomanip>

int main() {
    DWORD pid = 0;
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap != INVALID_HANDLE_VALUE) {
        PROCESSENTRY32W pe{ sizeof(pe) };
        if (Process32FirstW(snap, &pe)) {
            do {
                if (_wcsicmp(pe.szExeFile, L"OSRO.exe") == 0) {
                    pid = pe.th32ProcessID;
                    break;
                }
            } while (Process32NextW(snap, &pe));
        }
        CloseHandle(snap);
    }

    std::ofstream out("char_select_flags.txt", std::ios::trunc);
    if (!pid) {
        out << "OSRO.exe not found.\n";
        return 1;
    }

    HANDLE h = OpenProcess(PROCESS_VM_READ, FALSE, pid);
    if (!h) {
        out << "OpenProcess failed (" << GetLastError() << ")\n";
        return 1;
    }

    uint32_t modeMgr = 0;      // 0x0107AA84
    uint32_t charActive = 0;    // 0x010DCCBC
    uint32_t playerLoaded = 0;  // 0x010DFAEC
    uint32_t uiFlag = 0;        // 0x00F33F90
    uint8_t  slot = 0;          // 0x010DF5F6
    char     name[25] = {};     // 0x010DF5D8
    char     map[17] = {};      // 0x010D856C

    ReadProcessMemory(h, (LPCVOID)0x0107AA84, &modeMgr, 4, nullptr);
    ReadProcessMemory(h, (LPCVOID)0x010DCCBC, &charActive, 4, nullptr);
    ReadProcessMemory(h, (LPCVOID)0x010DFAEC, &playerLoaded, 4, nullptr);
    ReadProcessMemory(h, (LPCVOID)0x00F33F90, &uiFlag, 4, nullptr);
    ReadProcessMemory(h, (LPCVOID)0x010DF5F6, &slot, 1, nullptr);
    ReadProcessMemory(h, (LPCVOID)0x010DF5D8, name, 24, nullptr);
    ReadProcessMemory(h, (LPCVOID)0x010D856C, map, 16, nullptr);

    out << "PID: " << pid << "\n"
        << "  Mode (0x0107AA84)        : " << modeMgr << " (0x" << std::hex << modeMgr << std::dec << ")\n"
        << "  CharActive (0x010DCCBC)  : " << charActive << "\n"
        << "  PlayerLoaded (0x010DFAEC): " << playerLoaded << "\n"
        << "  UiFlag (0x00F33F90)      : " << uiFlag << "\n"
        << "  CharSlot (0x010DF5F6)    : " << (int)slot << "\n"
        << "  Name (0x010DF5D8)        : \"" << name << "\"\n"
        << "  Map (0x010D856C)         : \"" << map << "\"\n";

    CloseHandle(h);
    return 0;
}
