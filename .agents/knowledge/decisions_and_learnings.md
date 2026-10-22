# Architectural Decisions & Discovered Learnings

This document records the architectural choices, reverse-engineering discoveries, memory address analysis, and security principles established during the development of ORLogin.

---

## 1. Domain Discoveries (Ragnarok Online / OSRO Highrate)

### 1.1 Legacy "OnlineAddress" Disproven
- **Context**: Legacy ORTools referenced `OnlineAddress = 0x010A2FB0` and checked `value > 0` to determine if a client was in-game.
- **Discovery**: In the 2018-06-21 OSRO Highrate client, `0x010A2FB0` is a static heap pointer (`0x1E980BA0` / `513280928`) initialized at client startup. Testing `value > 0` caused false positives, reporting clients as "in game" even when sitting at the login prompt.
- **Solution**: Memory diffing between logged-out and in-game states revealed the true client mode manager at `0x0107AA84` (`CModeMgr`: `0` = Login/Select, `2` = InGame) and the in-game active entity flag at `0x010DCCBC` (`0` = Logged out, `1` = In-game).

### 1.2 Un-Zeroed String Memory Invariant
- **Discovery**: When returning to the login screen from the game world (via Character Select or Disconnect), the client **does not zero out** the character name (`0x010DF5D8`) or map name (`0x010D856C`).
- **Decision**: Never use character name or map presence alone to infer that a character is logged into the world. Always verify `InGameAddress (0x010DCCBC) == 1` and `ModeAddress (0x0107AA84) == 2` first.

### 1.3 Client Viewport Window Enumeration
- **Context**: Need to target the primary game viewport window for `PostMessage` / `SendInput`.
- **Discovery**: Direct3D wrappers and borderless hooks can cause `IsWindowVisible(hwnd)` to return `FALSE` or set non-standard owner hierarchies.
- **Decision**: Filter top-level windows strictly by matching `GetWindowThreadProcessId == targetPid` and client rect dimensions `width > 200 && height > 200`.

---

## 2. Desktop Framework & Architecture

### 2.1 Native C++17 + Dear ImGui
- **Rationale**: Minimal latency (<20ms startup), tiny footprint (<1MB executable), zero webview/node/runtime dependencies, static CRT `/MT` linking.
- **Renderer Cascade**: Direct3D 11 Hardware $\rightarrow$ Direct3D 11 WARP Software Rasterizer $\rightarrow$ Direct3D 9 legacy fallback. Guarantees presentation across all graphics hardware and VMs.

### 2.2 Security Architecture (Zero-Plaintext Invariant)
- **At Rest**: AES-256-GCM authenticated cipher with authenticated header metadata (AAD).
- **KDF**: PBKDF2-HMAC-SHA256 (600,000 iterations via Windows CNG `bcrypt.dll`) with 16-byte random salt.
- **In-Memory**: Non-pageable `VirtualAlloc(PAGE_READWRITE)` + `VirtualLock` pages wrapped in DPAPI `CryptProtectMemory`. Plaintext is only exposed in RAM for the few milliseconds required during typing, then wiped via RAII guard with `SecureZeroMemory`.
- **Session Auto-Lock**: `WM_WTSSESSION_CHANGE` hooks `WTS_SESSION_LOCK` to purge keys from RAM when Windows is locked (`Win+L`).
- **Screen Protection**: Optional `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` prevents window capture in screenshots, OBS, and Discord screen shares.
