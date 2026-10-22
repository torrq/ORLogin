# ORLogin

A secure, high-performance login helper for **Oldschool Ragnarok Online (OSRO)**, built specifically for OSRO Highrate.

---

## Features

- **LastPass-Style Encrypted Vault**:
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
- **Flexible Login Sequences**:
  - `ID + Tab + Password + Enter` (with optional automatic ID field clearing via `VK_END` + 24x `VK_BACK`).
  - `Password only + Enter` (for users leveraging the client's native "Save ID" option).
  - Supports both queue-based `PostMessageW` (works unfocused) and hardware `SendInput` methods with configurable key delays.
- **Dark & Light Themes**:
  - Instant toggle button supporting a custom Emerald & Mint dark theme and paper light theme.
  - Integrates with Windows 10/11 immersive title bar dark mode.
- **Privacy & Workstation Security**:
  - Automatically locks the vault and purges keys from RAM when your Windows session locks (`WTS_SESSION_LOCK` / `Win + L`).
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
