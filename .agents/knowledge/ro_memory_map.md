# Ragnarok Online (OSRO) Memory Map Reference

## Client Executable Information
- Target: `OSRO.exe` (Oldschool RO Highrate)
- Architecture: 32-bit x86 PE (TimeDateStamp: 2018-06-21)
- Base Address: `0x00400000` (Image size: `0x02299000`)
- ASLR: Disabled in the client binary, allowing fixed virtual address dereferencing.

## Verified Highrate Address Offsets

| Property | Virtual Address | Type | Verified Behavior |
|---|---|---|---|
| `OnlineAddress` | `0x010A2FB0` | uint32 | **Primary ORTools Online Indicator**: **`> 0`** when character is logged in world. **`0`** when sitting at login screen. |
| `InGameAddress` | `0x010DCCBC` | uint32 | Fallback in-game world indicator. Can be 0 in certain instanced maps (e.g. queue/arena `prt_q`). |
| `ModeAddress` | `0x0107AA84` | uint32 | Client mode manager. 0 at login screen, non-zero in world/game. |
| `LoginFocusAddress` | `0x0107A13C` | uint32 | **`5`** when User ID input field is focused. **`0`** when Password input field is focused. Corroborated with caret Y-pos (`106` vs `138`). |
| `PlayerLoaded` | `0x010DFAEC` | uint32 | **`1`** when player character entity is spawned into the world. **`0`** at login screen. |
| `NameAddress` | `0x010DF5D8` | char[24] | Character name in ANSI (CP-1252 / Western). |
| `MapAddress` | `0x010D856C` | char[16] | Current map name (e.g. `prt_q`, `prontera`). |
| `CharSlotAddress`| `0x010DF5F6` | uint8 | Currently selected character slot (0-indexed byte, offset `+0x1E` from `NameAddress`). |
