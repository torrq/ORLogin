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

### 3.4 Bottom-Anchored Status Bar Invariant
- **Context**: On screens with compact content (e.g. Vault Creation, Vault Unlocking), the status bar floated directly under the card in the upper-middle of the window.
- **Decision**: In `RenderStatusBar`, calculate `targetY = GetWindowHeight() - statusBarH`. If `GetCursorPosY() < targetY`, explicitly assign `SetCursorPosY(targetY)` so the status bar is anchored to the window bottom across all application states.

### 3.5 Advanced Timing Modal & Key Injection Polish
- **Decision**: Rather than cluttering the primary UI with multiple timing sliders, standard delays are housed in a dedicated `Advanced Settings` modal accessible via `[ Advanced... ]` in the settings header.
- **Configurable Delays**:
  - `keyDelayMs` (1-60 ms, def 15 ms): Character typing speed.
  - `enterDelayMs` (20-1000 ms, def 200 ms): Pause between 1st Return (credentials) and 2nd Return (server select).
  - `tabDelayMs` (5-200 ms, def 25 ms): Pause after Tab field switching and auto-alignment.
  - `clearDelayMs` (1-30 ms, def 3 ms): Backspace speed when wiping the ID field.
  - `keyHoldMs` (2-50 ms, def 10 ms): Hardware key-down hold duration before key-up.
- **Keystroke Synthesis**: For `VK_RETURN`, `PostMessage` dispatches both `WM_KEYDOWN` and `WM_CHAR` (`0x0D`), ensuring modal dialogs that consume character events process the Enter key reliably.

---

## 4. Hardening & Security Remediation (v0.5)

### 4.1 Authenticated Security Settings (Vault Payload v2)
- **Problem**: Storing security timings (`keyDelayMs`, `autoLockMinutes`, `inputMethod`) in plaintext JSON allowed local unprivileged users or malware to alter delay parameters or disable lockouts without unlocking the vault.
- **Solution**: Upgraded vault payload format to version 2. Security-critical parameters are serialized, authenticated, and encrypted inside `ORLogin.vault` via AES-256-GCM. Unauthenticated `ORLogin.settings.json` is restricted strictly to cosmetic/startup preferences (`theme`, `hotkeyVk`, `hotkeyMods`, `hideFromCapture`). Backward compatibility for payload v1 is maintained.

### 4.2 Storage Location: Standard LocalAppData vs. Portable Mode
- **Decision**: Default vault and settings storage moved to `%LOCALAPPDATA%\ORLogin\` to isolate user vaults within Windows user profile DACLs.
- **Automatic Migration**: On launch, if `ORLogin.vault` exists in the executable folder but not yet in LocalAppData, it is automatically migrated with its backup and settings.
- **Portable Support**: If `portable.flag` exists in the executable folder (or when compiled with `-DORLOGIN_PORTABLE=ON`), ORLogin operates in portable mode and keeps all files alongside the binary. The status bar displays `[Portable]` as confirmation.

### 4.3 Master Password Policy (8-Character Minimum)
- **Decision**: Vault creation and password rotation enforce a minimum length of 8 characters (blocking weak passwords while respecting user ergonomic preferences), backed by live entropy/class estimation.

### 4.4 DLL Search-Order Hijacking Protection
- **Context**: `ORLogin.manifest` specifies `requireAdministrator`. Elevated processes running from user-writable directories are vulnerable to DLL planting attacks (e.g. malicious `d3d11.dll` placed next to the `.exe`).
- **Solution**:
  1. Early startup in `wWinMain` calls `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32)` and enables `BASE_SEARCH_PATH_ENABLE_SAFE_SEARCHMODE`.
  2. MSVC linker configured with `/DEPENDENTLOADFLAG:0x800` to burn the System32 search constraint directly into the PE Load Config directory.

### 4.5 Focus Check & In-Game Typing Hard-Block
- **Context**: In-game typing risked dumping passwords into game public chat or trade windows if hotkey was triggered accidentally. Furthermore, when using `SendInput`, focus shifting away from the game during typing would leak remaining password characters into whatever window was active.
- **Solution**:
  1. Unconditionally block typing when `ClientInfo::state == ClientState::InGame`. The user toggle to disable this check is eliminated.
  2. In `Typer::Run`, every character and keystroke under `SendInput` verifies `GetForegroundWindow() == job.hwnd`. If focus changes at any point during typing, the loop aborts immediately and memory buffers are purged.
  3. Validate executable image name (`OSRO.exe`) via `QueryFullProcessImageNameW`.

### 4.6 Single-Instance Mutex & Compiler Mitigations
- **Single-Instance**: Named mutex `Local\ORLogin_SingleInstance_Mutex_B6D83D71` prevents duplicate processes from conflicting over the vault or hotkey; activates and restores the existing window if launched again.
- **Compiler / Linker Mitigations**: Built with Control Flow Guard (`/guard:cf`), CET Shadow Stack (`/CETCOMPAT`), High-Entropy 64-bit ASLR (`/HIGHENTROPYVA`, `/DYNAMICBASE`), and DEP (`/NXCOMPAT`).



