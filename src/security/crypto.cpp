#include "security/crypto.h"

#include <windows.h>
#include <bcrypt.h>

#ifndef NT_SUCCESS
#define NT_SUCCESS(Status) (((NTSTATUS)(Status)) >= 0)
#endif
#ifndef STATUS_AUTH_TAG_MISMATCH
#define STATUS_AUTH_TAG_MISMATCH ((NTSTATUS)0xC000A002L)
#endif

namespace Crypto {

namespace {

struct AlgHandle {
    BCRYPT_ALG_HANDLE h = nullptr;
    ~AlgHandle() { if (h) BCryptCloseAlgorithmProvider(h, 0); }
};

struct KeyHandle {
    BCRYPT_KEY_HANDLE h = nullptr;
    ~KeyHandle() { if (h) BCryptDestroyKey(h); }
};

bool OpenAesGcm(AlgHandle& alg, KeyHandle& key, const uint8_t keyBytes[kKeySize]) {
    if (!NT_SUCCESS(BCryptOpenAlgorithmProvider(&alg.h, BCRYPT_AES_ALGORITHM, nullptr, 0))) return false;
    if (!NT_SUCCESS(BCryptSetProperty(alg.h, BCRYPT_CHAINING_MODE,
                                      (PUCHAR)BCRYPT_CHAIN_MODE_GCM,
                                      sizeof(BCRYPT_CHAIN_MODE_GCM), 0))) return false;
    if (!NT_SUCCESS(BCryptGenerateSymmetricKey(alg.h, &key.h, nullptr, 0,
                                               (PUCHAR)keyBytes, (ULONG)kKeySize, 0))) return false;
    return true;
}

} // namespace

bool Random(uint8_t* out, size_t len) {
    return NT_SUCCESS(BCryptGenRandom(nullptr, out, (ULONG)len, BCRYPT_USE_SYSTEM_PREFERRED_RNG));
}

bool Pbkdf2Sha256(const uint8_t* password, size_t passwordLen,
                  const uint8_t* salt, size_t saltLen,
                  uint32_t iterations,
                  uint8_t* outKey, size_t outKeyLen) {
    AlgHandle alg;
    if (!NT_SUCCESS(BCryptOpenAlgorithmProvider(&alg.h, BCRYPT_SHA256_ALGORITHM, nullptr,
                                                BCRYPT_ALG_HANDLE_HMAC_FLAG))) return false;
    // CNG rejects a NULL password pointer even with length 0.
    static const uint8_t kEmpty = 0;
    NTSTATUS st = BCryptDeriveKeyPBKDF2(alg.h,
                                        (PUCHAR)(password ? password : &kEmpty), (ULONG)passwordLen,
                                        (PUCHAR)salt, (ULONG)saltLen,
                                        iterations,
                                        outKey, (ULONG)outKeyLen, 0);
    return NT_SUCCESS(st);
}

bool AesGcmEncrypt(const uint8_t key[kKeySize], const uint8_t nonce[kNonceSize],
                   const uint8_t* aad, size_t aadLen,
                   const uint8_t* plaintext, size_t plaintextLen,
                   uint8_t* ciphertext, uint8_t tag[kTagSize]) {
    AlgHandle alg;
    KeyHandle hKey;
    if (!OpenAesGcm(alg, hKey, key)) return false;

    BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO info;
    BCRYPT_INIT_AUTH_MODE_INFO(info);
    info.pbNonce    = (PUCHAR)nonce;
    info.cbNonce    = (ULONG)kNonceSize;
    info.pbAuthData = (PUCHAR)aad;
    info.cbAuthData = (ULONG)aadLen;
    info.pbTag      = tag;
    info.cbTag      = (ULONG)kTagSize;

    ULONG written = 0;
    NTSTATUS st = BCryptEncrypt(hKey.h, (PUCHAR)plaintext, (ULONG)plaintextLen, &info,
                                nullptr, 0, ciphertext, (ULONG)plaintextLen, &written, 0);
    return NT_SUCCESS(st) && written == plaintextLen;
}

Result AesGcmDecrypt(const uint8_t key[kKeySize], const uint8_t nonce[kNonceSize],
                     const uint8_t* aad, size_t aadLen,
                     const uint8_t* ciphertext, size_t ciphertextLen,
                     const uint8_t tag[kTagSize], uint8_t* plaintext) {
    AlgHandle alg;
    KeyHandle hKey;
    if (!OpenAesGcm(alg, hKey, key)) return Result::Error;

    BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO info;
    BCRYPT_INIT_AUTH_MODE_INFO(info);
    info.pbNonce    = (PUCHAR)nonce;
    info.cbNonce    = (ULONG)kNonceSize;
    info.pbAuthData = (PUCHAR)aad;
    info.cbAuthData = (ULONG)aadLen;
    info.pbTag      = (PUCHAR)tag;
    info.cbTag      = (ULONG)kTagSize;

    ULONG written = 0;
    NTSTATUS st = BCryptDecrypt(hKey.h, (PUCHAR)ciphertext, (ULONG)ciphertextLen, &info,
                                nullptr, 0, plaintext, (ULONG)ciphertextLen, &written, 0);
    if (st == STATUS_AUTH_TAG_MISMATCH) {
        SecureZeroMemory(plaintext, ciphertextLen);
        return Result::AuthFailed;
    }
    if (!NT_SUCCESS(st) || written != ciphertextLen) {
        SecureZeroMemory(plaintext, ciphertextLen);
        return Result::Error;
    }
    return Result::Ok;
}

} // namespace Crypto
