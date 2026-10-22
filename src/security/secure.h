#pragma once

// Secure memory primitives.
//
//  * SecureAllocator / SecureBytes: page-backed buffers that are VirtualLock'd
//    (best effort: kept out of the pagefile) and wiped with SecureZeroMemory on free.
//    std::vector is used instead of std::string on purpose: no small-string
//    optimisation means secret bytes never live inline in an unwiped object.
//
//  * ProtectedSecret: a secret held encrypted in RAM via CryptProtectMemory
//    (SAME_PROCESS). It is only decrypted into a SecureBytes for the brief moment
//    it is needed (saving the vault, typing into the client, reveal-on-hold).

#include <windows.h>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <new>
#include <vector>

template <typename T>
struct SecureAllocator {
    using value_type = T;

    SecureAllocator() noexcept = default;
    template <class U> SecureAllocator(const SecureAllocator<U>&) noexcept {}

    T* allocate(std::size_t n) {
        std::size_t bytes = (n == 0 ? 1 : n) * sizeof(T);
        void* p = VirtualAlloc(nullptr, bytes, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
        if (!p) throw std::bad_alloc();
        VirtualLock(p, bytes); // best effort; failure is non-fatal
        return static_cast<T*>(p);
    }

    void deallocate(T* p, std::size_t n) noexcept {
        if (!p) return;
        std::size_t bytes = (n == 0 ? 1 : n) * sizeof(T);
        SecureZeroMemory(p, bytes);
        VirtualUnlock(p, bytes);
        VirtualFree(p, 0, MEM_RELEASE);
    }

    template <class U> bool operator==(const SecureAllocator<U>&) const noexcept { return true; }
    template <class U> bool operator!=(const SecureAllocator<U>&) const noexcept { return false; }
};

using SecureBytes = std::vector<uint8_t, SecureAllocator<uint8_t>>;
using SecureWide  = std::vector<wchar_t, SecureAllocator<wchar_t>>;

// Length of a NUL-terminated string stored in a fixed SecureBytes edit buffer.
inline size_t SecureStrLen(const SecureBytes& buf) {
    if (buf.empty()) return 0;
    const void* z = std::memchr(buf.data(), 0, buf.size());
    return z ? static_cast<size_t>(static_cast<const uint8_t*>(z) - buf.data()) : buf.size();
}

// Zero the whole buffer in place (keeps its size).
inline void SecureWipe(SecureBytes& buf) {
    if (!buf.empty()) SecureZeroMemory(buf.data(), buf.size());
}

// Copy the NUL-terminated contents of an edit buffer into an exact-size SecureBytes.
inline SecureBytes SecureFromEditBuffer(const SecureBytes& buf) {
    size_t n = SecureStrLen(buf);
    SecureBytes out;
    out.reserve(n == 0 ? 1 : n);
    out.assign(buf.begin(), buf.begin() + n);
    return out;
}

class ProtectedSecret {
public:
    ProtectedSecret() = default;
    ProtectedSecret(const uint8_t* data, size_t len) { Set(data, len); }

    // Replace contents. `data` is copied, then encrypted in place.
    void Set(const uint8_t* data, size_t len);
    void Set(const SecureBytes& data) { Set(data.data(), data.size()); }

    // Decrypt into a fresh secure buffer (exact length). Wiped automatically on destruction.
    SecureBytes Reveal() const;

    void   Clear();
    size_t Size() const { return m_len; }
    bool   Empty() const { return m_len == 0; }

private:
    SecureBytes m_blob; // CRYPTPROTECTMEMORY_BLOCK_SIZE-padded ciphertext
    size_t      m_len = 0;
};

namespace Secure {
    // Raise the process working-set minimum so VirtualLock has room for our pages.
    void InitProcessMemory();

    // Wipe Dear ImGui's internal copies of text from InputText widgets (active text
    // buffer, revert buffer, undo stack, deactivation backup). Must be called while
    // no InputText is active, e.g. right after a password form is submitted.
    void WipeImGuiTextState();
}
