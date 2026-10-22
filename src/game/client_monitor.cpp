#include "game/client_monitor.h"

#include <tlhelp32.h>
#include <set>

const char* ClientStateText(ClientState s) {
    switch (s) {
    case ClientState::NoAccess: return "No access";
    case ClientState::AtLogin:  return "Login / Char select";
    case ClientState::InGame:   return "In game";
    }
    return "?";
}

const char* LoginFocusText(LoginFieldFocus f) {
    switch (f) {
    case LoginFieldFocus::UserId:   return "User ID";
    case LoginFieldFocus::Password: return "Password";
    case LoginFieldFocus::Unknown:  return "Unknown";
    }
    return "?";
}

const ClientInfo* ClientMonitor::FindByPid(DWORD pid) const {
    for (const auto& c : m_clients)
        if (c.pid == pid) return &c;
    return nullptr;
}

void ClientMonitor::Refresh() {
    std::set<DWORD> alive;

    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap != INVALID_HANDLE_VALUE) {
        PROCESSENTRY32W pe{};
        pe.dwSize = sizeof(pe);
        if (Process32FirstW(snap, &pe)) {
            do {
                if (_wcsicmp(pe.szExeFile, m_profile.processName) == 0) alive.insert(pe.th32ProcessID);
            } while (Process32NextW(snap, &pe));
        }
        CloseHandle(snap);
    }

    // Drop readers for processes that are gone.
    for (auto it = m_readers.begin(); it != m_readers.end();) {
        if (!alive.count(it->first) || it->second->HasExited()) it = m_readers.erase(it);
        else ++it;
    }

    std::vector<ClientInfo> next;
    next.reserve(alive.size());
    for (DWORD pid : alive) {
        auto& reader = m_readers[pid];
        if (!reader) {
            reader = std::make_unique<ProcessReader>();
            reader->Open(pid);
        }

        ClientInfo info;
        info.pid  = pid;
        info.hwnd = FindMainWindow(pid);
        info.state = reader->IsOpen() ? ReadState(*reader, info.charName, info.map, info.charSlot, info.loginFocus) : ClientState::NoAccess;
        if (info.state == ClientState::NoAccess) reader.reset(); // retry opening next refresh
        next.push_back(std::move(info));
    }
    m_clients = std::move(next);
}

// In-game vs Login state detection based on ORTools memory architecture:
// OnlineAddress (>0 when online in HR, ==3571961 in MR), supplemented by inGameAddress/modeAddress.
ClientState ClientMonitor::ReadState(ProcessReader& r, std::string& name, std::string& map, uint8_t& outSlot, LoginFieldFocus& outFocus) const {
    uint32_t inGame = 0;
    uint32_t mode = 0;
    uint32_t online = 0;

    bool hasInGame = (m_profile.inGameAddress != 0) && r.ReadU32(m_profile.inGameAddress, inGame);
    bool hasMode   = (m_profile.modeAddress != 0) && r.ReadU32(m_profile.modeAddress, mode);
    bool hasOnline = (m_profile.onlineAddress != 0) && r.ReadU32(m_profile.onlineAddress, online);

    if (!hasInGame && !hasMode && !hasOnline) {
        outFocus = LoginFieldFocus::Unknown;
        outSlot  = 0;
        return ClientState::NoAccess;
    }

    // InGame if ORTools online pointer is active (>0), or character entity is active (inGame == 1 || mode == 2)
    bool isOnline = (hasOnline && online > 0) || (hasInGame && inGame == 1) || (hasMode && mode == 2);

    if (isOnline) {
        name = r.ReadString(m_profile.nameAddress, 24);
        map  = r.ReadString(m_profile.mapAddress, 16);
        outSlot = 0;
        if (m_profile.charSlotAddress != 0) {
            r.Read(m_profile.charSlotAddress, &outSlot, sizeof(outSlot));
        }
        outFocus = LoginFieldFocus::Unknown;
        return ClientState::InGame;
    }

    name.clear();
    map.clear();
    outSlot = 0;

    outFocus = LoginFieldFocus::Unknown;
    if (m_profile.loginFocusAddress != 0) {
        uint32_t focusVal = 0;
        if (r.ReadU32(m_profile.loginFocusAddress, focusVal)) {
            if (focusVal == 5) {
                outFocus = LoginFieldFocus::UserId;
            } else if (focusVal == 0) {
                outFocus = LoginFieldFocus::Password;
            }
        }
    }

    return ClientState::AtLogin;
}

HWND ClientMonitor::FindMainWindow(DWORD pid) {
    struct Ctx { DWORD pid; HWND found; } ctx{ pid, nullptr };
    EnumWindows([](HWND h, LPARAM lp) -> BOOL {
        auto* c = reinterpret_cast<Ctx*>(lp);
        DWORD wpid = 0;
        GetWindowThreadProcessId(h, &wpid);
        if (wpid == c->pid && IsWindow(h)) {
            RECT rc{};
            if (GetClientRect(h, &rc)) {
                int w = rc.right - rc.left;
                int hSize = rc.bottom - rc.top;
                if (w > 200 && hSize > 200) {
                    c->found = h;
                    return FALSE; // Found game viewport window
                }
            }
        }
        return TRUE;
    }, reinterpret_cast<LPARAM>(&ctx));
    return ctx.found;
}

