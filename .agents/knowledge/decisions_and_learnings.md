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

### 1.4 Login Input Field Focus Detection (`0x0107A13C`)
- **Problem**: If the game client had the Password input box focused (via prior click or Tab), triggering a standard `ID + Tab + Pass` sequence would inadvertently type the username into the password field.
- **Discovery**: 3-way differential memory scanning across 380MB of committed pages (User ID $\rightarrow$ Password $\rightarrow$ User ID) isolated static module address `0x0107A13C` in `OSRO.exe`:
  - `5` (`0x05`) = User ID field currently focused (Control ID 5).
  - `0` (`0x00`) = Password field currently focused (Control ID 0).
  - Corroborated with active input caret screen coordinates: Y = `106` (User ID) vs Y = `138` (Password, exactly 32px vertical offset).
- **Decision**: In `Typer`, inspect `ClientInfo::loginFocus`. If the target client is focused on the wrong field for the requested sequence, automatically dispatch a single <kbd>Tab</kbd> to align focus before credential dispatch.
- **Double Return Sequence**: Send two consecutive <kbd>Return</kbd> taps separated by a 150ms pause to submit credentials and immediately confirm the Service/Server select prompt into Character Selection.

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

---

## 3. UI/UX Design & Typography Invariants

### 3.1 Extended Glyph Ranges for Game Entity Rendering
- **Discovery**: When reading player character names encoded in CP-1252, special characters like `™` (`\x99` $\rightarrow$ `U+2122`) caused Dear ImGui's default font atlas to emit `'?'` fallbacks (`Torq?`).
- **Decision**: Load native Windows `segoeui.ttf` with custom glyph ranges spanning:
  - `0x0020 - 0x00FF` (Latin 1 Supplement)
  - `0x2000 - 0x206F` (General Punctuation)
  - `0x2100 - 0x214F` (Letterlike Symbols containing `™` `U+2122`)
  - `0x2190 - 0x21FF` (Arrows)
  - `0x25A0 - 0x25FF` (Geometric Shapes containing `●` bullets)

### 3.2 Action Column & Layout Sizing
- **Context**: Actions column with `Send`, `Edit`, `Delete` buttons clipped the right rounded edge of the `Delete` button when column width was `175px`.
- **Decision**: Actions column width is established at `220px` to guarantee comfortable margins across all UI scales and padding options.
- **Settings Layout**: Split bottom panel into a fixed 310px width for running clients and remaining width for settings, employing a 2-column table grid (`SettingsLayoutGrid`) to avoid horizontal overflow or trailing character cutoff.

### 3.3 Vector UI Icons
- **Decision**: In place of raster image assets or heavy font icon sets (e.g. FontAwesome), interactive icons (such as the Vault Lock padlock) are rendered procedurally via `ImDrawList` primitives. This ensures crisp vector scaling at any DPI, zero external file dependencies, and interactive theme/hover state color shifts.

