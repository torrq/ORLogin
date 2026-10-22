#pragma once

// Enumerates running OSRO clients and classifies their state from memory.

#include <windows.h>
#include <map>
#include <memory>
#include <string>
#include <vector>

#include "game/addresses.h"
#include "game/process_reader.h"

enum class ClientState {
    NoAccess, // could not open/read the process (not elevated? anti-cheat?)
    AtLogin,  // not in-game: login screen OR server/char select (see note in .cpp)
    InGame,
};

struct ClientInfo {
    DWORD       pid   = 0;
    HWND        hwnd  = nullptr;
    ClientState state = ClientState::NoAccess;
    std::string charName;
    std::string map;
};

const char* ClientStateText(ClientState s);

class ClientMonitor {
public:
    explicit ClientMonitor(const ServerProfile& profile) : m_profile(profile) {}

    // Re-enumerates processes and refreshes every client's state.
    void Refresh();

    const std::vector<ClientInfo>& Clients() const { return m_clients; }
    const ClientInfo* FindByPid(DWORD pid) const;
    const ServerProfile& Profile() const { return m_profile; }

private:
    ClientState ReadState(ProcessReader& r, std::string& name, std::string& map) const;
    static HWND FindMainWindow(DWORD pid);

    const ServerProfile& m_profile;
    std::map<DWORD, std::unique_ptr<ProcessReader>> m_readers;
    std::vector<ClientInfo> m_clients;
};
