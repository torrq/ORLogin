# OSRO Login Helper (ORLogin)

A secure, high-performance login helper for **Oldschool Ragnarok Online (OSRO)**, built specifically for Highrate (and Midrate).

---

## Features

- **Encrypted Local Vault**:
  - Saved accounts and passwords are encrypted with **AES-256-GCM** using keys derived via **PBKDF2-HMAC-SHA256** (600,000 iterations via Windows CNG `bcrypt.dll`) with cryptographic random salt and per-save nonces.
  - Zero plaintext on disk. The vault file (`ORLogin.vault`) is atomically replaced upon save with automated `.bak` backups.
- **In-Memory Hardening**:
  - Master keys and decrypted passwords reside only in non-pageable `VirtualLock` memory pages, wiped using `SecureZeroMemory` immediately after use.
  - Sensitive strings are shielded in RAM with Windows DPAPI (`CryptProtectMemory`) until the exact moment of transmission.
- **Real-Time Client Memory Awareness**:
  - Non-invasive `ReadProcessMemory` inspection reads the live game mode (`0x0107AA84`) and character world entity flag (`0x010DCCBC`).
  - Reliably distinguishes **Login Screen / Char Select** from **In-Game World**, overcoming the un-zeroed string memory trap common to RO clients.
- **Safe Hotkey Trigger**:
  - Configurable global hotkey (defaults to `Ctrl + Shift + L`).
  - Automatically identifies the active account (or defaults to the single account).
  - Precondition checks ensure credentials are only typed when the focused client is sitting at the login screen, preventing accidental chat leaks during gameplay.
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
  - Optional window display protection (`WDA_EXCLUDEFROMCAPTURE`) to prevent screen capture / streaming leaks on OBS and Discord.
  - Progressive rate-limiting delay on failed master password attempts.
- **Cascading Zero-Dependency Rendering**:
  - 3-tier graphics fallback: Direct3D 11 Hardware $\rightarrow$ Direct3D 11 WARP Software Rasterizer $\rightarrow$ Direct3D 9 legacy fallback.
  - Standalone, statically linked (`/MT`) native C++17 binary (~900 KB). Zero webview, Node, or runtime dependencies.

---

## Requirements

- **Operating System**: Windows 7 SP1, 8, 8.1, 10, or 11 (x64).
- **Privileges**: Run as **Administrator** (required to read memory and send window messages to elevated game clients; enforced via application manifest).

---

## Building from Source

1. Open PowerShell or Command Prompt in the repository root.
2. Run `build.bat`:
   ```cmd
   .\build.bat
   ```
3. The executable will be compiled to `build\Release\ORLogin.exe`.

---

## Documentation & Knowledge Base

Detailed architectural decisions, reverse-engineered memory specifications, and agent skills are documented in the [`.agents/`](.agents/) directory:
- [`.agents/skills/ro-client-login-automation/SKILL.md`](.agents/skills/ro-client-login-automation/SKILL.md): Complete guide on RO memory structures, login detection, window discovery, and secure credential handling.
- [`.agents/knowledge/decisions_and_learnings.md`](.agents/knowledge/decisions_and_learnings.md): Reverse-engineering notes and discovered invariants.
- [`.agents/knowledge/ro_memory_map.md`](.agents/knowledge/ro_memory_map.md): Authoritative virtual address table for OSRO clients.
