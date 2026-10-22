#include "security/vault.h"

#include <windows.h>
#include <cstring>
#include <limits>

namespace {

constexpr char     kMagic[4]       = { 'O', 'R', 'L', 'V' };
constexpr uint16_t kFormatVersion  = 1;
constexpr uint16_t kKdfPbkdf2      = 1;
constexpr size_t   kHeaderSize     = 44;
constexpr uint32_t kPayloadVersion = 1;
constexpr uint32_t kMaxCiphertext  = 16u * 1024u * 1024u;
constexpr uint32_t kMaxAccounts    = 10000;
constexpr uint32_t kMaxFieldLen    = 4096;

void PutU16(uint8_t* p, uint16_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); }
void PutU32(uint8_t* p, uint32_t v) { for (int i = 0; i < 4; ++i) p[i] = (uint8_t)(v >> (8 * i)); }
uint16_t GetU16(const uint8_t* p) { return (uint16_t)(p[0] | (p[1] << 8)); }
uint32_t GetU32(const uint8_t* p) { return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24); }

void AppendU32(SecureBytes& b, uint32_t v) {
    uint8_t tmp[4];
    PutU32(tmp, v);
    b.insert(b.end(), tmp, tmp + 4);
}

void AppendField(SecureBytes& b, const uint8_t* data, size_t len) {
    AppendU32(b, (uint32_t)len);
    if (len) b.insert(b.end(), data, data + len);
}

struct Reader {
    const SecureBytes& b;
    size_t pos = 0;
    bool ReadU32(uint32_t& v) {
        if (b.size() - pos < 4) return false;
        v = GetU32(b.data() + pos);
        pos += 4;
        return true;
    }
    bool ReadField(const uint8_t*& data, uint32_t& len) {
        if (!ReadU32(len) || len > kMaxFieldLen || b.size() - pos < len) return false;
        data = b.data() + pos;
        pos += len;
        return true;
    }
};

void WipeString(std::string& s) {
    if (!s.empty()) SecureZeroMemory(&s[0], s.size());
    s.clear();
    s.shrink_to_fit();
}

} // namespace

Vault::Vault(std::filesystem::path path) : m_path(std::move(path)) {}

Vault::~Vault() { Lock(); }

bool Vault::Exists() const {
    std::error_code ec;
    return std::filesystem::is_regular_file(m_path, ec);
}

const char* Vault::StatusText(Status s) {
    switch (s) {
    case Status::Ok:            return "OK";
    case Status::NotFound:      return "Vault file not found";
    case Status::WrongPassword: return "Wrong master password (or the vault was modified)";
    case Status::Corrupt:       return "Vault file is corrupt or unsupported";
    case Status::IoError:       return "Could not read/write the vault file";
    case Status::CryptoError:   return "Windows cryptography error";
    }
    return "Unknown error";
}

void Vault::NormalizeActive() {
    const int n = (int)m_accounts.size();
    if (n == 0)            m_active = -1;
    else if (n == 1)       m_active = 0;
    else if (m_active >= n) m_active = n - 1;
    else if (m_active < -1) m_active = -1;
}

// ───────────────────────────── Key / file helpers ─────────────────────────────

Vault::Status Vault::DeriveKey(const SecureBytes& master, const uint8_t salt[Crypto::kSaltSize],
                               uint32_t iterations, SecureBytes& outKey) const {
    outKey.reserve(Crypto::kKeySize);
    outKey.assign(Crypto::kKeySize, 0);
    if (!Crypto::Pbkdf2Sha256(master.data(), master.size(), salt, Crypto::kSaltSize,
                              iterations, outKey.data(), outKey.size())) {
        SecureWipe(outKey);
        return Status::CryptoError;
    }
    return Status::Ok;
}

Vault::Status Vault::ReadFile(std::vector<uint8_t>& out) const {
    HANDLE h = CreateFileW(m_path.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr,
                           OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) {
        DWORD err = GetLastError();
        return (err == ERROR_FILE_NOT_FOUND || err == ERROR_PATH_NOT_FOUND) ? Status::NotFound : Status::IoError;
    }
    LARGE_INTEGER size{};
    if (!GetFileSizeEx(h, &size) || size.QuadPart > (LONGLONG)(kMaxCiphertext + kHeaderSize + Crypto::kTagSize)) {
        CloseHandle(h);
        return Status::Corrupt;
    }
    out.resize((size_t)size.QuadPart);
    DWORD read = 0;
    BOOL ok = out.empty() ? TRUE : ::ReadFile(h, out.data(), (DWORD)out.size(), &read, nullptr);
    CloseHandle(h);
    if (!ok || read != out.size()) return Status::IoError;
    return Status::Ok;
}

Vault::Status Vault::WriteFileAtomic(const std::vector<uint8_t>& data) const {
    std::filesystem::path tmp = m_path; tmp += L".tmp";
    std::filesystem::path bak = m_path; bak += L".bak";

    HANDLE h = CreateFileW(tmp.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS,
                           FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) return Status::IoError;

    DWORD written = 0;
    BOOL ok = WriteFile(h, data.data(), (DWORD)data.size(), &written, nullptr) && written == data.size();
    ok = ok && FlushFileBuffers(h);
    CloseHandle(h);
    if (!ok) {
        DeleteFileW(tmp.c_str());
        return Status::IoError;
    }

    if (Exists()) CopyFileW(m_path.c_str(), bak.c_str(), FALSE); // keep previous generation

    if (!MoveFileExW(tmp.c_str(), m_path.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) {
        DeleteFileW(tmp.c_str());
        return Status::IoError;
    }
    return Status::Ok;
}

Vault::Status Vault::DecryptWithKey(const std::vector<uint8_t>& file, const SecureBytes& key,
                                    SecureBytes& outPlain, uint8_t outSalt[Crypto::kSaltSize],
                                    uint32_t& outIterations) const {
    if (file.size() < kHeaderSize + Crypto::kTagSize) return Status::Corrupt;
    const uint8_t* p = file.data();
    if (std::memcmp(p, kMagic, 4) != 0) return Status::Corrupt;
    if (GetU16(p + 4) != kFormatVersion || GetU16(p + 6) != kKdfPbkdf2) return Status::Corrupt;

    outIterations = GetU32(p + 8);
    std::memcpy(outSalt, p + 12, Crypto::kSaltSize);
    const uint8_t* nonce = p + 28;
    const uint32_t ctLen = GetU32(p + 40);
    if (ctLen > kMaxCiphertext || file.size() != kHeaderSize + ctLen + Crypto::kTagSize) return Status::Corrupt;

    outPlain.reserve(ctLen == 0 ? 1 : ctLen);
    outPlain.assign(ctLen, 0);
    Crypto::Result r = Crypto::AesGcmDecrypt(key.data(), nonce, p, kHeaderSize,
                                             p + kHeaderSize, ctLen,
                                             p + kHeaderSize + ctLen, outPlain.data());
    if (r == Crypto::Result::AuthFailed) return Status::WrongPassword;
    if (r != Crypto::Result::Ok)         return Status::CryptoError;
    return Status::Ok;
}

// ───────────────────────────── Payload ─────────────────────────────

SecureBytes Vault::SerializePayload() const {
    size_t estimate = 12;
    for (const auto& a : m_accounts) estimate += 12 + a.label.size() + a.username.size() + a.password.Size();

    SecureBytes b;
    b.reserve(estimate);
    AppendU32(b, kPayloadVersion);
    AppendU32(b, (uint32_t)(int32_t)m_active);
    AppendU32(b, (uint32_t)m_accounts.size());
    for (const auto& a : m_accounts) {
        AppendField(b, (const uint8_t*)a.label.data(), a.label.size());
        AppendField(b, (const uint8_t*)a.username.data(), a.username.size());
        SecureBytes pw = a.password.Reveal();
        AppendField(b, pw.data(), pw.size());
    }
    return b;
}

bool Vault::DeserializePayload(const SecureBytes& plain) {
    Reader r{ plain };
    uint32_t version = 0, active = 0, count = 0;
    if (!r.ReadU32(version) || version != kPayloadVersion) return false;
    if (!r.ReadU32(active) || !r.ReadU32(count) || count > kMaxAccounts) return false;

    std::vector<Account> accounts;
    accounts.reserve(count);
    for (uint32_t i = 0; i < count; ++i) {
        const uint8_t* d = nullptr;
        uint32_t len = 0;
        Account a;
        if (!r.ReadField(d, len)) return false;
        a.label.assign((const char*)d, len);
        if (!r.ReadField(d, len)) return false;
        a.username.assign((const char*)d, len);
        if (!r.ReadField(d, len)) return false;
        a.password.Set(d, len);
        accounts.push_back(std::move(a));
    }
    if (r.pos != plain.size()) return false;

    m_accounts = std::move(accounts);
    m_active   = (int)(int32_t)active;
    NormalizeActive();
    return true;
}

// ───────────────────────────── Public API ─────────────────────────────

Vault::Status Vault::Create(const SecureBytes& master) {
    Lock();
    uint8_t salt[Crypto::kSaltSize];
    if (!Crypto::Random(salt, sizeof(salt))) return Status::CryptoError;

    SecureBytes key;
    Status s = DeriveKey(master, salt, kDefaultIterations, key);
    if (s != Status::Ok) return s;

    std::memcpy(m_salt, salt, sizeof(salt));
    m_iterations = kDefaultIterations;
    m_key.Set(key);
    m_accounts.clear();
    m_active   = -1;
    m_unlocked = true;

    s = Save();
    if (s != Status::Ok) Lock();
    return s;
}

Vault::Status Vault::Unlock(const SecureBytes& master) {
    Lock();
    std::vector<uint8_t> file;
    Status s = ReadFile(file);
    if (s != Status::Ok) return s;

    if (file.size() < kHeaderSize) return Status::Corrupt;
    uint32_t iterations = GetU32(file.data() + 8);
    if (iterations < kMinIterations || iterations > kMaxIterations) return Status::Corrupt;
    uint8_t salt[Crypto::kSaltSize];
    std::memcpy(salt, file.data() + 12, sizeof(salt));

    SecureBytes key;
    s = DeriveKey(master, salt, iterations, key);
    if (s != Status::Ok) return s;

    SecureBytes plain;
    uint8_t fileSalt[Crypto::kSaltSize];
    uint32_t fileIters = 0;
    s = DecryptWithKey(file, key, plain, fileSalt, fileIters);
    if (s != Status::Ok) return s;

    if (!DeserializePayload(plain)) return Status::Corrupt;

    std::memcpy(m_salt, salt, sizeof(salt));
    m_iterations = iterations;
    m_key.Set(key);
    m_unlocked = true;
    return Status::Ok;
}

Vault::Status Vault::Save() {
    if (!m_unlocked) return Status::CryptoError;

    SecureBytes plain = SerializePayload();
    SecureBytes key   = m_key.Reveal();
    if (key.size() != Crypto::kKeySize) return Status::CryptoError;

    std::vector<uint8_t> out(kHeaderSize + plain.size() + Crypto::kTagSize);
    uint8_t* p = out.data();
    std::memcpy(p, kMagic, 4);
    PutU16(p + 4, kFormatVersion);
    PutU16(p + 6, kKdfPbkdf2);
    PutU32(p + 8, m_iterations);
    std::memcpy(p + 12, m_salt, Crypto::kSaltSize);
    if (!Crypto::Random(p + 28, Crypto::kNonceSize)) return Status::CryptoError;
    PutU32(p + 40, (uint32_t)plain.size());

    if (!Crypto::AesGcmEncrypt(key.data(), p + 28, p, kHeaderSize,
                               plain.data(), plain.size(),
                               p + kHeaderSize, p + kHeaderSize + plain.size())) {
        return Status::CryptoError;
    }
    return WriteFileAtomic(out);
}

Vault::Status Vault::ChangeMasterPassword(const SecureBytes& current, const SecureBytes& next) {
    if (!m_unlocked) return Status::CryptoError;

    // Verify the current password against the stored key (constant-time compare).
    SecureBytes check;
    Status s = DeriveKey(current, m_salt, m_iterations, check);
    if (s != Status::Ok) return s;
    SecureBytes key = m_key.Reveal();
    uint8_t diff = (uint8_t)(key.size() ^ check.size());
    for (size_t i = 0; i < key.size() && i < check.size(); ++i) diff |= key[i] ^ check[i];
    if (diff != 0) return Status::WrongPassword;

    uint8_t salt[Crypto::kSaltSize];
    if (!Crypto::Random(salt, sizeof(salt))) return Status::CryptoError;
    SecureBytes newKey;
    s = DeriveKey(next, salt, kDefaultIterations, newKey);
    if (s != Status::Ok) return s;

    // Swap in new key material; roll back if the write fails.
    uint8_t  oldSalt[Crypto::kSaltSize];
    uint32_t oldIters = m_iterations;
    std::memcpy(oldSalt, m_salt, sizeof(oldSalt));

    std::memcpy(m_salt, salt, sizeof(salt));
    m_iterations = kDefaultIterations;
    m_key.Set(newKey);

    s = Save();
    if (s != Status::Ok) {
        std::memcpy(m_salt, oldSalt, sizeof(oldSalt));
        m_iterations = oldIters;
        m_key.Set(key);
    }
    return s;
}

void Vault::Lock() {
    for (auto& a : m_accounts) {
        WipeString(a.label);
        WipeString(a.username);
        a.password.Clear();
    }
    m_accounts.clear();
    m_accounts.shrink_to_fit();
    m_key.Clear();
    SecureZeroMemory(m_salt, sizeof(m_salt));
    m_active   = -1;
    m_unlocked = false;
}
