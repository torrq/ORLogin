# Ragnarok Online (OSRO) Memory Map Reference

## Client Executable Information
- Target: `OSRO.exe` (Oldschool RO Highrate)
- Architecture: 32-bit x86 PE (TimeDateStamp: 2018-06-21)
- Base Address: `0x00400000` (Image size: `0x02299000`)
- ASLR: Disabled in the client binary, allowing fixed virtual address dereferencing.

## Verified Highrate Address Offsets

| Property | Virtual Address | Type | Verified Behavior |
|---|---|---|---|
| `InGameAddress` | `0x010DCCBC` | uint32 | **`1`** when active inside the game world. **`0`** when logged out / at login screen / at character selection. |
| `ModeAddress` | `0x0107AA84` | uint32 | **`2`** when in Game Mode (`CGameMode`). **`0`** when at Login Screen or Character Selection. |
| `PlayerLoaded` | `0x010DFAEC` | uint32 | **`1`** when player character entity is spawned into the world. **`0`** when logged out. |
| `NameAddress` | `0x010DF5D8` | char[24] | Character name in ANSI (CP-1252). Note: The game client does **not** zero this out when logging out to login screen; check `InGameAddress == 1` first. |
| `MapAddress` | `0x010D856C` | char[16] | Current map name (e.g. `prontera`). Retains previous map until new game entry. |
| `CharSlotAddress`| `0x010DF5F6` | uint8 | Currently selected character slot (0-indexed). |
| `LegacyOnline` | `0x010A2FB0` | uint32 | **DO NOT USE for login state.** This is a static heap pointer (`0x1E980BA0`) that is `> 0` even at the login screen. |
