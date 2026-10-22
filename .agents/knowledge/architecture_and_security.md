# ORLogin Architecture & Security Design

## 1. Zero-Plaintext Security Architecture
- **At Rest**: Credentials are saved in `ORLogin.vault` using AES-256-GCM.
- **Key Derivation**: PBKDF2-HMAC-SHA256 with 600,000 iterations and a 16-byte cryptographically secure random salt generated via Windows CNG `BCryptGenRandom`.
- **Integrity & Authenticated Data (AAD)**: The header bytes (magic `ORLV`, version, KDF type, iterations, salt, nonce, and ciphertext length) are supplied as authenticated data to the GCM cipher. Tampering with any byte or metadata triggers an auth tag mismatch and rejection.
- **In-Memory Secrets**:
  - `SecureAllocator` allocates non-pageable memory via `VirtualAlloc(PAGE_READWRITE)` and locks it in the process working set via `VirtualLock`.
  - On destruction or lock, all memory is explicitly wiped via `SecureZeroMemory` before being unlocked and released.
  - Plaintext passwords in memory are shielded by Windows DPAPI `CryptProtectMemory(..., CRYPTPROTECTMEMORY_SAME_PROCESS)`.
  - When typing into the game, the password is decrypted into a locked `SecureBytes` struct for only the few milliseconds necessary for transmission, then wiped immediately by a RAII guard.
  - ImGui text state, undo stack, and clipboard/deactivation backups are wiped using `Secure::WipeImGuiTextState()`.
- **System Locks**: Registered via `WTSRegisterSessionNotification`. When Windows locks (`WTS_SESSION_LOCK` / Win+L), `OnSessionLock()` triggers an immediate wipe and locks the vault.

## 2. Process & Memory Introspection
- Target Process: `OSRO.exe` (32-bit x86 executable).
- Memory Addresses (from ORTools Highrate):
  - `OnlineAddress`: `0x010A2FB0` (u32; `> 0` means in-game).
  - `NameAddress`: `0x010DF5D8` (char[24]; character name).
  - `MapAddress`: `0x010D856C` (char[16]; map name).
  - `CharSlotAddress`: `0x010DF5F6` (u8; selected slot).
- Precondition Check: Prior to transmitting credentials, the target process memory is read. If `OnlineAddress > 0` (character is already logged in) and `requireLoginScreen` is active, the hotkey aborts to prevent sending passwords into the chatbox or game world.

## 3. Input Injection
- Supports both `PostMessageW` (direct window message queue injection using `WM_KEYDOWN`/`WM_KEYUP`/`WM_CHAR` with proper scan codes) and `SendInput` (hardware-level keyboard event injection).
- Dedicated background worker thread ensures the UI remains completely fluid while performing delays between keystrokes.
- Modifiers released check ensures `Ctrl` or `Shift` held during hotkey execution does not pollute the typed characters.
