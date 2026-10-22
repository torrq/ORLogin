#pragma once

// Thin wrappers over Windows CNG (bcrypt.dll). No third-party crypto.

#include <cstddef>
#include <cstdint>

namespace Crypto {

constexpr size_t kKeySize   = 32; // AES-256
constexpr size_t kSaltSize  = 16;
constexpr size_t kNonceSize = 12; // GCM standard nonce
constexpr size_t kTagSize   = 16;

enum class Result { Ok, AuthFailed, Error };

// Cryptographically secure random bytes (system-preferred RNG).
bool Random(uint8_t* out, size_t len);

// PBKDF2-HMAC-SHA256.
bool Pbkdf2Sha256(const uint8_t* password, size_t passwordLen,
                  const uint8_t* salt, size_t saltLen,
                  uint32_t iterations,
                  uint8_t* outKey, size_t outKeyLen);

// AES-256-GCM. `ciphertext` must have room for `plaintextLen` bytes.
bool AesGcmEncrypt(const uint8_t key[kKeySize], const uint8_t nonce[kNonceSize],
                   const uint8_t* aad, size_t aadLen,
                   const uint8_t* plaintext, size_t plaintextLen,
                   uint8_t* ciphertext, uint8_t tag[kTagSize]);

// Returns AuthFailed on wrong key or any tampering. `plaintext` must have room for `ciphertextLen` bytes.
Result AesGcmDecrypt(const uint8_t key[kKeySize], const uint8_t nonce[kNonceSize],
                     const uint8_t* aad, size_t aadLen,
                     const uint8_t* ciphertext, size_t ciphertextLen,
                     const uint8_t tag[kTagSize], uint8_t* plaintext);

} // namespace Crypto
