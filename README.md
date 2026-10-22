# OSRO Login Helper (ORLogin)

A secure, high-performance login helper for **Oldschool Ragnarok Online (OSRO)**, built specifically for Highrate (and Midrate).

---

<img width="862" height="709" alt="image" src="https://github.com/user-attachments/assets/3ad52a8f-6083-4148-986b-aab784dfc8a2" />

## Features

- **Encrypted Local Vault**:
  - Saved accounts and passwords are encrypted with **AES-256-GCM** using keys derived via **PBKDF2-HMAC-SHA256** (600,000 iterations via Windows CNG `bcrypt.dll`) with cryptographic random salt and per-save nonces.
  - Zero plaintext on disk. The vault file (`ORLogin.vault`) is atomically replaced upon save with automated `.bak` backups.
  - **Authenticated Security Settings (Payload v2)**: Key delays, input injection methods, auto-lock timeouts, and safety configurations are cryptographically authenticated and encrypted inside the vault payload. Plaintext JSON stores only cosmetic/startup preferences (theme, hotkey, capture exclusion).
  - Minimum 8-character master password policy enforced with visual strength estimation.
  - Automatic rotation backup cleanup: updating the master password immediately refreshes `.bak` with new ciphertext, ensuring no stale old-key ciphertexts linger on disk.
- **In-Memory Hardening**:
  - Master keys and decrypted passwords reside only in non-pageable `VirtualLock` memory pages, wiped using `SecureZeroMemory` immediately after use.
  - Sensitive strings are shielded in RAM with Windows DPAPI (`CryptProtectMemory`) until the exact moment of transmission.
- **Targeting & Typing Safety**:
  - **In-Game Lockout**: Unconditionally blocks typing whenever a client is detected in-game (displaying the character name and current map), preventing accidental credential leaks into game chat or trade windows.
  - **Dynamic Focus Guard**: When using hardware `SendInput`, the background typer validates that the game client is actively in the foreground before dispatching each character. If focus shifts away (e.g. user clicks another window or notification steals focus), typing is aborted immediately and memory buffers are purged.
  - **Process Identity Pinning**: Validates process image name (`OSRO.exe`) via `QueryFullProcessImageNameW` before dispatching any credentials.
- **Binary Hardening & Exploit Mitigations**:
  - **DLL Hijacking Immunity**: Calls `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32)` and safe search path modes at startup; PE header contains `/DEPENDENTLOADFLAG:0x800` to prevent malicious DLL planting in application folders.
  - **Advanced Process Mitigations**: Compiled with Control Flow Guard (`/guard:cf`), Shadow Stack compatibility (`/CETCOMPAT`), High-Entropy ASLR (`/HIGHENTROPYVA`, `/DYNAMICBASE`), and Data Execution Prevention (`/NXCOMPAT`).
  - **Single-Instance Enforcement**: Named local mutex (`ORLogin_SingleInstance_Mutex`) ensures only one instance runs at a time, automatically focusing and restoring the existing window if launched again.
- **Real-Time Client Memory Awareness**:
  - Non-invasive `ReadProcessMemory` inspection reads the live game mode (`0x0107AA84`), online state (`0x010A2FB0`), character world entity flag (`0x010DCCBC`), character name (`0x010DF5D8`), character slot (`0x010DF5F6`), and current map name (`0x00F31D68`).
  - Reliably distinguishes **Login Screen / Char Select** from **In-Game World**, overcoming the un-zeroed string memory trap common to RO clients.
- **Safe Hotkey Trigger**:
  - Configurable global hotkey (defaults to `Ctrl + Shift + L`).
  - Automatically identifies the active account (or defaults to the single account).
  - Precondition checks ensure credentials are only typed when the focused client is sitting at the login screen.
- **Flexible Login Sequences & Focus Alignment**:
  - `ID + Tab + Password + Enter x2` (with optional automatic ID field clearing via `VK_END` + 24x `VK_BACK`).
  - **Memory Focus Auto-Alignment**: Reads live input focus (`0x0107A13C`); if the client has the Password field focused, automatically sends a <kbd>Tab</kbd> to align back to User ID before typing.
  - **Double Return Sequence**: Hits Return twice (with accompanying `WM_CHAR` and configurable pause) to submit credentials and immediately select the server prompt, taking you directly to Character Select.
  - **Advanced Timing Configuration**: Dedicated modal to fine-tune individual delays (character typing, double-Enter pause, Tab focus switch, field clear backspace, and hardware hold duration).
  - `Password only + Enter x2` (for users leveraging the client's native "Save ID" option).
  - Supports both queue-based `PostMessageW` (works unfocused) and hardware `SendInput` methods with configurable key delays.
- **Modern Themes & Native Typography**:
  - Instant toggle button supporting a high-contrast Obsidian & Cyan dark theme and clean Studio paper light theme.
  - Full UTF-8 and CP-1252 symbol coverage (including `™` Trademark sign in character names) rendered with Windows Segoe UI.
  - Integrates with Windows 10/11 immersive title bar dark mode.
- **Privacy & Workstation Security**:
  - Automatically locks the vault and purges keys from RAM when your Windows session locks (`WTS_SESSION_LOCK` / `Win + L`).
  - One-click vector padlock icon to instantly lock the vault.
  - Configurable idle auto-lock timer.
  - Optional window display protection (`WDA_EXCLUDEFROMCAPTURE`) to prevent screen capture / streaming leaks on OBS and Discord (disabled by default).
  - Progressive rate-limiting delay on failed master password attempts.
- **Cascading Zero-Dependency Rendering**:
  - 3-tier graphics fallback: Direct3D 11 Hardware $\rightarrow$ Direct3D 11 WARP Software Rasterizer $\rightarrow$ Direct3D 9 legacy fallback.
  - Standalone, statically linked (`/MT`) native C++17 binary (~940 KB). Zero webview, Node, or runtime dependencies.

---

## Standard vs. Portable Mode

ORLogin supports both standard installed usage and fully portable USB drive execution:

### 1. Standard Mode (Default)
- **Vault & Settings Path**: `%LOCALAPPDATA%\ORLogin\`
- **Automatic Migration**: If upgrading from an older version that stored files in the application folder, ORLogin automatically migrates `ORLogin.vault`, `ORLogin.vault.bak`, and `ORLogin.settings.json` to `%LOCALAPPDATA%\ORLogin\` on first launch.
- **Security Advantage**: Vault files reside in your personal Windows user profile, preventing unprivileged users or shared folder guests from accessing the encrypted vault.

### 2. Portable Mode
- **Vault & Settings Path**: The same folder as `ORLogin.exe`.
- **How to Activate at Runtime**:
  Simply create an empty file named `portable.flag` in the same directory as `ORLogin.exe`.
  When detected, ORLogin reads and writes `ORLogin.vault` and `ORLogin.settings.json` strictly in the executable folder. The bottom status bar will display `[Portable]` to confirm portable mode is active.
- **How to Build Portable by Default**:
  Pass `-DORLOGIN_PORTABLE=ON` during CMake configuration.

---

## Requirements

- **Operating System**: Windows 7 SP1, 8, 8.1, 10, or 11 (x64).
- **Privileges**: Run as **Administrator** (required to read memory and send window messages to elevated game clients; enforced via application manifest).

---

## Building from Source

### Standard Build
1. Open PowerShell or Command Prompt in the repository root.
2. Run `build.bat`:
   ```cmd
   .\build.bat
   ```
   Or via CMake directly:
   ```cmd
   cmake -B build -DCMAKE_BUILD_TYPE=Release
   cmake --build build --config Release
   ```
3. The compiled binary will be placed at `build\Release\ORLogin.exe`.

### Portable Release Build
To compile a binary that defaults to portable mode without requiring `portable.flag`:
```cmd
cmake -B build -DORLOGIN_PORTABLE=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
```

---

## Documentation & Knowledge Base

Detailed architectural decisions, reverse-engineered memory specifications, and agent skills are documented in the [`.agents/`](.agents/) directory:
- [`.agents/skills/ro-client-login-automation/SKILL.md`](.agents/skills/ro-client-login-automation/SKILL.md): Complete guide on RO memory structures, login detection, window discovery, and secure credential handling.
- [`.agents/knowledge/decisions_and_learnings.md`](.agents/knowledge/decisions_and_learnings.md): Reverse-engineering notes and discovered invariants.
- [`.agents/knowledge/ro_memory_map.md`](.agents/knowledge/ro_memory_map.md): Authoritative virtual address table for OSRO clients.
