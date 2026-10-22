#pragma once

// Encrypted account vault.
//
// File format (little-endian), version 1:
//   off  size  field
//   0    4     magic "ORLV"
//   4    2     format version (1)
//   6    2     KDF id (1 = PBKDF2-HMAC-SHA256)
//   8    4     KDF iterations
//   12   16    salt
//   28   12    AES-GCM nonce (fresh random on every save)
//   40   4     ciphertext length
//   44   n     ciphertext (AES-256-GCM)
//   44+n 16    GCM tag
// Bytes [0, 44) are authenticated as AAD, so the header cannot be tampered with either.
//
// The plaintext payload (labels, usernames, passwords, active index) is serialised
// in a length-prefixed binary layout straight into locked/wiped memory – it is never
// turned into JSON or std::string, and never touches disk unencrypted.

#include <cstdint>
#include <filesystem>
#include <string>
#include <vector>

#include "security/crypto.h"
#include "security/secure.h"
#include "settings.h"

struct Account {
    std::string     label;     // optional nickname shown in the UI
    std::string     username;
    ProtectedSecret password;  // encrypted in RAM; Reveal() only when needed
};

class Vault {
public:
    enum class Status { Ok, NotFound, WrongPassword, Corrupt, IoError, CryptoError };

    static constexpr uint32_t kDefaultIterations = 600000; // OWASP 2023 recommendation for PBKDF2-SHA256
    static constexpr uint32_t kMinIterations     = 100000;
    static constexpr uint32_t kMaxIterations     = 50000000;

    explicit Vault(std::filesystem::path path);
    ~Vault();

    Vault(const Vault&) = delete;
    Vault& operator=(const Vault&) = delete;

    const std::filesystem::path& Path() const { return m_path; }
    bool Exists() const;

    // Creates a brand-new empty vault protected by `master` and writes it to disk.
    Status Create(const SecureBytes& master, const VaultSettings& initialSettings = {});

    // Derives the key from `master` and decrypts the vault file.
    Status Unlock(const SecureBytes& master);

    // Re-encrypts the in-memory accounts with a fresh nonce and atomically replaces the file.
    Status Save();

    // Verifies `current`, then re-keys the vault with a new random salt.
    Status ChangeMasterPassword(const SecureBytes& current, const SecureBytes& next);

    // Wipes the key and all decrypted data from memory.
    void Lock();

    bool IsUnlocked() const { return m_unlocked; }

    std::vector<Account>&       Accounts()       { return m_accounts; }
    const std::vector<Account>& Accounts() const { return m_accounts; }

    const VaultSettings& GetSettings() const { return m_settings; }
    void                 SetSettings(const VaultSettings& s) { m_settings = s; }
    bool                 HasLoadedSettings() const { return m_hasSettings; }

    int  ActiveIndex() const { return m_active; }
    void SetActiveIndex(int idx) { m_active = idx; NormalizeActive(); }
    void NormalizeActive(); // single account => active; clamps out-of-range

    static const char* StatusText(Status s);

private:
    Status DeriveKey(const SecureBytes& master, const uint8_t salt[Crypto::kSaltSize],
                     uint32_t iterations, SecureBytes& outKey) const;
    Status ReadFile(std::vector<uint8_t>& out) const;
    Status WriteFileAtomic(const std::vector<uint8_t>& data) const;
    Status DecryptWithKey(const std::vector<uint8_t>& file, const SecureBytes& key,
                          SecureBytes& outPlain, uint8_t outSalt[Crypto::kSaltSize],
                          uint32_t& outIterations) const;

    SecureBytes SerializePayload() const;
    bool        DeserializePayload(const SecureBytes& plain);

    std::filesystem::path m_path;
    bool                  m_unlocked = false;
    ProtectedSecret       m_key;
    uint8_t               m_salt[Crypto::kSaltSize] = {};
    uint32_t              m_iterations = kDefaultIterations;
    std::vector<Account>  m_accounts;
    int                   m_active = -1;
    VaultSettings         m_settings;
    bool                  m_hasSettings = false;
};
