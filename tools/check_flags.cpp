#include <windows.h>
#include <tlhelp32.h>
#include <iostream>
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
    if (!pid) return 1;

    HANDLE h = OpenProcess(PROCESS_VM_READ, FALSE, pid);
    if (!h) return 2;

    uint32_t modeMgr = 0;   // 0x0107AA84
    uint32_t charFlag = 0;  // 0x010DCCBC
    uint32_t playerFlag = 0;// 0x010DFAEC
    uint32_t uiFlag = 0;    // 0x00F33F90

    ReadProcessMemory(h, (LPCVOID)0x0107AA84, &modeMgr, 4, nullptr);
    ReadProcessMemory(h, (LPCVOID)0x010DCCBC, &charFlag, 4, nullptr);
    ReadProcessMemory(h, (LPCVOID)0x010DFAEC, &playerFlag, 4, nullptr);
    ReadProcessMemory(h, (LPCVOID)0x00F33F90, &uiFlag, 4, nullptr);

    std::cout << "PID " << pid << " Live Flags:\n"
              << "  Mode (0x0107AA84)       : " << modeMgr << "\n"
              << "  CharActive (0x010DCCBC) : " << charFlag << "\n"
              << "  PlayerLoaded (0x010DFAEC): " << playerFlag << "\n"
              << "  UiFlag (0x00F33F90)     : " << uiFlag << "\n";
    CloseHandle(h);
    return 0;
}
