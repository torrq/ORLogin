---
name: ro-client-login-automation
description: Domain knowledge, reverse-engineered memory structures, login screen detection algorithms, input injection mechanics, and secure credential handling for Ragnarok Online (RO/OSRO) automation tools.
---

# Ragnarok Online (RO) Client Memory & Login Automation Skill

This skill provides domain knowledge, memory structures, reverse-engineering discoveries, Win32 window message injection techniques, and security standards for building automation and login assistance tools for Ragnarok Online (specifically OSRO Highrate / 2018-06-21 Ragexe clients).

---

## 1. RO Client Memory Architecture & State Detection

### 1.1 The Login State Trap: Un-Zeroed Memory Buffers
- **Critical Discovery**: When a player logs out or disconnects from the game world back to Character Selection or the Login Screen, the RO game client (`OSRO.exe` / `Ragexe`) **does not zero out** the active character buffer or map buffer.
- `NameAddress` (`0x010DF5D8`) and `MapAddress` (`0x010D856C`) retain the previous session's character name and map string indefinitely while sitting at the login screen.
- Checking `strlen(name) > 0` alone will cause false positives (reporting the client is in-game when it is actually at the login screen).

### 1.2 The "OnlineAddress" Pointer & State Verification
- In OSRO Highrate, `OnlineAddress` (`0x010A2FB0`) is the primary indicator used by ORTools (`loginValue > 0`).
- Differential analysis against raw memory snapshots (`snap_pass.bin`, `snap_user1.bin`) confirms that at the login screen, `0x010A2FB0 == 0`. When in-game, it points to the active session heap structure (`> 0`).
- `CharacterSlotAddress` (`0x010DF5F6`) is located at offset `+0x1E` from `NameAddress` (`0x010DF5D8`), containing the 0-indexed byte of the active character slot.
- `InGameAddress` (`0x010DCCBC`) can drop to 0 in certain instanced maps (e.g. `prt_q`). Therefore, checking `onlineAddress > 0` (or `inGame == 1 || mode == 2`) provides robust, leak-proof detection.

### 1.3 Verified Client State Matrix
To reliably distinguish whether a client is at the **Login Screen**, **Character Selection**, or **In-Game World**, inspect the following verified virtual addresses:

| Client State | `OnlineAddress` (`0x010A2FB0`) | `PlayerLoaded` (`0x010DFAEC`) | `ModeAddress` (`0x0107AA84`) | `InGameAddress` (`0x010DCCBC`) | Description |
|---|:---:|:---:|:---:|:---:|---|
| **Login Screen** | `0` | `0` | `0` | `0` | Client is waiting at username/password prompt. Safe to type credentials. |
| **Character Select** | `0` | `0` | `0` | `0` | Account session authenticated; viewing character slots. |
| **In-Game World** | `> 0` | `1` | `!= 0` | `1` (or `0` in `prt_q`) | Player character entity is spawned into the map. Automation should block login typing. |

```cpp
bool IsClientAtLoginScreen(uint32_t mode, uint32_t inGame) {
    // Mode 0 = CLoginMode / CSelectMode, inGame 0 = no active world entity
    return (mode == 0 && inGame == 0);
}

bool IsClientInGame(uint32_t mode, uint32_t inGame) {
    // Mode 2 = CGameMode, inGame 1 = world character active
    return (mode == 2 || inGame == 1);
}
```

### 1.4 Login Input Field Focus Detection (`0x0107A13C`)
- When at the login screen, static address `0x0107A13C` (`uint32_t`) identifies which input field currently holds active keyboard focus:
  - **`5`**: **User ID** input field focused.
  - **`0`**: **Password** input field focused.
- Active input caret screen Y-coordinates corroborate this state:
  - User ID input box caret: Y = `106` (`0x6A`)
  - Password input box caret: Y = `138` (`0x8A`, 32px vertical offset)
- **Auto-Alignment**: Before executing `ID + Tab + Pass`, read `0x0107A13C`. If `0` (Password), dispatch a single <kbd>Tab</kbd> to align focus to User ID first. For `Password Only`, if `5` (User ID), dispatch a <kbd>Tab</kbd> to align to Password.

---

## 2. Window Enumeration & Viewport Discovery

### 2.1 Direct3D Wrapper & Window Visibility Quirks
- Standard `FindWindow` or `EnumWindows` filtering on `IsWindowVisible(hwnd)` or `GetWindow(hwnd, GW_OWNER) == NULL` frequently fails on RO clients utilizing borderless window hooks, ddraw wrappers, or DirectX overlays (e.g. dgvoodoo, ddraw.dll).
- The client window may be an owned child or have non-standard extended styles (`WS_EX_LAYERED`, `WS_EX_TOOLWINDOW`).

### 2.2 Robust Viewport Matching Algorithm
Enumerate all windows associated with the target process ID, testing solely for window validity and client dimension thresholds:
```cpp
HWND FindGameWindow(DWORD targetPid) {
    HWND found = nullptr;
    EnumWindows([](HWND h, LPARAM lp) -> BOOL {
        DWORD wpid = 0;
        GetWindowThreadProcessId(h, &wpid);
        auto* outHwnd = reinterpret_cast<HWND*>(lp);
        if (wpid == targetPid && IsWindow(h)) {
            RECT rc{};
            if (GetClientRect(h, &rc)) {
                int width  = rc.right - rc.left;
                int height = rc.bottom - rc.top;
                // RO game viewports are at least 640x480; threshold > 200 filters out dummy helper windows
                if (width > 200 && height > 200) {
                    *outHwnd = h;
                    return FALSE; // Found primary viewport
                }
            }
        }
        return TRUE;
    }, reinterpret_cast<LPARAM>(&found));
    return found;
}
```

---

## 3. Keystroke & Credential Input Injection

### 3.1 PostMessage vs SendInput
- **`PostMessageW` (Recommended)**: Direct message delivery to the client's window message queue (`WM_KEYDOWN`, `WM_KEYUP`, `WM_CHAR`).
  - Does **not** require the game window to be the active foreground window.
  - Generates proper `lParam` bitfields containing virtual scan codes (`MapVirtualKeyW(vk, MAPVK_VK_TO_VSC)`).
  - Gepard Shield anti-cheat handles `PostMessageW` without driver-level hook interception when sent with realistic scan codes.
- **`SendInput`**: Injects hardware-level scancodes into the OS event stream.
  - Requires `SetForegroundWindow(hwnd)`.
  - Must ensure any user-held modifier keys (e.g. `Ctrl`, `Shift` from the trigger hotkey) are released before dispatching characters, otherwise `Ctrl + char` shortcuts may fire inside the game.

### 3.2 Login Form Field Reset & Double Return Sequence
When typing into the RO login form:
1. **Focus Alignment**: Check `LoginFocusAddress` (`0x0107A13C`). If focus is on Password (`0`), send `VK_TAB` first to bring focus to User ID.
2. **Clear ID Field**: Tap `VK_END` to place the caret at the end of the ID field, then send 24 consecutive `VK_BACK` taps (RO account usernames have a hard cap of 24 characters).
3. **Type Credentials**: Type username string $\rightarrow$ Tap `VK_TAB` $\rightarrow$ Type password string.
4. **Double Return**: Tap `VK_RETURN` (submits credentials), pause for ~150ms, then tap `VK_RETURN` a second time (confirms Service/Server selection and immediately advances into Character Selection).

---

## 4. Security Standards for In-Memory Credentials

1. **Non-Pageable Memory**:
   - Store master keys and decrypted passwords only in memory pages locked with `VirtualLock` to prevent swapping to `pagefile.sys`.
2. **Zero Plaintext on Disk**:
   - Encrypt configuration vaults using authenticated ciphers (AES-256-GCM) with key derivation via PBKDF2-HMAC-SHA256 (600,000+ iterations).
3. **DPAPI RAM Protection**:
   - Keep in-memory strings protected via `CryptProtectMemory(..., CRYPTPROTECTMEMORY_SAME_PROCESS)`. Unprotect only for the immediate microsecond duration needed to transmit the string.
4. **Immediate Zeroing**:
   - Use `SecureZeroMemory` on all buffers, edit fields, and Dear ImGui internal text edit states (`ImGuiInputTextState`) immediately upon submission or form closure.
5. **Session Lock Purging**:
   - Hook `WM_WTSSESSION_CHANGE` (`WTS_SESSION_LOCK`). Instantly wipe keys and lock vaults when the user locks Windows (`Win + L`).

---

## 5. Character Set & Font Atlas Ranges

### 5.1 RO Client Character Encodings
- Ragnarok Online clients store character and map strings in Windows-1252 (CP-1252) for Western/English servers or EUC-KR / CP-949 for Korean clients.
- Player character names frequently contain extended symbols such as the Trademark sign (`™`, byte `0x99` in CP-1252 $\rightarrow$ Unicode `U+2122`).
- Convert memory strings using `MultiByteToWideChar(1252, ...)` (falling back to `CP_ACP`) to `std::wstring`, then `WideCharToMultiByte(CP_UTF8, ...)` for UTF-8.

### 5.2 Dear ImGui Font Atlas Requirements
- Dear ImGui's default font (ProggyClean) and standard Latin range (`0x0020 - 0x00FF`) do **not** include Letterlike Symbols (`0x2100 - 0x214F`). Missing glyphs fall back to `'?'` (e.g. rendering `Torq™` as `Torq?`).
- Always load a modern system font (e.g. `C:\Windows\Fonts\segoeui.ttf`) with explicit glyph ranges:
  - `0x0020 - 0x00FF` (Basic Latin + Latin Supplement)
  - `0x2000 - 0x206F` (General Punctuation)
  - `0x2100 - 0x214F` (Letterlike Symbols — contains `U+2122` `™`)
  - `0x2190 - 0x21FF` (Arrows)
  - `0x25A0 - 0x25FF` (Geometric Shapes — contains `●` bullets)

