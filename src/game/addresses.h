#pragma once

// Static memory addresses for OSRO clients.
// Discovered via live memory diffing of OSRO Highrate (2018-06-21 client).

#include <cstdint>

struct ServerProfile {
    const char*    label;            // UI name
    const char*    shortLabel;       // badge
    const wchar_t* processName;      // client executable
    uint32_t       inGameAddress;    // u32; 1 when in-game world, 0 when at login screen / char select
    uint32_t       modeAddress;      // u32; 2 when in game, 0 when at login screen / char select
    uint32_t       onlineAddress;    // u32; ORTools online pointer (>0 when online)
    uint32_t       loginFocusAddress;// u32; 5 when User ID focused, 0 when Password focused
    uint32_t       nameAddress;      // char[24] character name
    uint32_t       mapAddress;       // char[16] current map
    uint32_t       charSlotAddress;  // u8 selected character slot (0-based)
};

namespace Servers {

inline constexpr ServerProfile HighRate{
    "Highrate", "HR", L"OSRO.exe",
    /* inGameAddress     */ 0x010DCCBC, // 1 = InGame, 0 = LoggedOut / Login Screen
    /* modeAddress       */ 0x0107AA84, // 2 = InGame, 0 = LoggedOut / Login Screen
    /* onlineAddress     */ 0x010A2FB0, // ORTools online address (>0 when online)
    /* loginFocusAddress */ 0x0107A13C, // 5 = User ID focused, 0 = Password focused
    /* name              */ 0x010DF5D8,
    /* map               */ 0x010D856C,
    /* charSlot          */ 0x010DF5F6,
};

// Mid Rate profile (ORTools addresses)
inline constexpr ServerProfile MidRate{
    "Midrate", "MR", L"OSRO Midrate.exe",
    /* inGameAddress     */ 0x00E884B1,
    /* modeAddress       */ 0x00E884B1,
    /* onlineAddress     */ 0x00E884B1,
    /* loginFocusAddress */ 0x00000000,
    /* name              */ 0x00E91C00,
    /* map               */ 0x00E8ABD4,
    /* charSlot          */ 0x00E91C1E,
};

} // namespace Servers
