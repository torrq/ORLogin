#include "security/secure.h"

#include <dpapi.h>

#include "vendor/imgui/imgui.h"
#include "vendor/imgui/imgui_internal.h"

// Pull in the full STB_TexteditState definition (imgui_internal.h only forward-declares it),
// exactly the way imgui_widgets.cpp does, so we can wipe its undo buffer.
namespace ImStb {
#include "vendor/imgui/imstb_textedit.h"
}

// ───────────────────────────── ProtectedSecret ─────────────────────────────

void ProtectedSecret::Set(const uint8_t* data, size_t len) {
    Clear();
    if (!data || len == 0) return;

    const size_t block  = CRYPTPROTECTMEMORY_BLOCK_SIZE;
    const size_t padded = ((len + block - 1) / block) * block;

    m_blob.reserve(padded);
    m_blob.assign(padded, 0);
    std::memcpy(m_blob.data(), data, len);
    m_len = len;

    if (!CryptProtectMemory(m_blob.data(), static_cast<DWORD>(padded), CRYPTPROTECTMEMORY_SAME_PROCESS)) {
        // Never keep an unprotected copy around if protection failed.
        Clear();
        throw std::bad_alloc();
    }
}

SecureBytes ProtectedSecret::Reveal() const {
    SecureBytes out;
    if (m_len == 0) return out;

    out.reserve(m_blob.size());
    out.assign(m_blob.begin(), m_blob.end());
    if (!CryptUnprotectMemory(out.data(), static_cast<DWORD>(out.size()), CRYPTPROTECTMEMORY_SAME_PROCESS)) {
        SecureWipe(out);
        out.clear();
        return out;
    }
    // Zero the padding tail before shrinking the logical size (capacity is wiped on free anyway).
    SecureZeroMemory(out.data() + m_len, out.size() - m_len);
    out.resize(m_len);
    return out;
}

void ProtectedSecret::Clear() {
    SecureWipe(m_blob);
    m_blob.clear();
    m_blob.shrink_to_fit();
    m_len = 0;
}

// ───────────────────────────── Process / ImGui ─────────────────────────────

namespace Secure {

void InitProcessMemory() {
    // Soft limits (default flags) – just gives VirtualLock headroom.
    SetProcessWorkingSetSize(GetCurrentProcess(), 8 * 1024 * 1024, 128 * 1024 * 1024);
}

static void WipeVec(ImVector<char>& v) {
    if (v.Data && v.Capacity > 0) SecureZeroMemory(v.Data, static_cast<size_t>(v.Capacity));
    v.clear();
}

void WipeImGuiTextState() {
    ImGuiContext* ctx = ImGui::GetCurrentContext();
    if (!ctx) return;
    ImGuiContext& g = *ctx;

    ImGuiInputTextState& s = g.InputTextState;
    WipeVec(s.TextA);
    WipeVec(s.TextToRevertTo);
    WipeVec(s.CallbackTextBackup);
    if (s.Stb) SecureZeroMemory(s.Stb, sizeof(ImStbTexteditState));
    s.TextLen = 0;
    s.ID = 0; // forces full re-initialisation the next time any InputText activates

    WipeVec(g.InputTextDeactivatedState.TextA);
    g.InputTextDeactivatedState.ID = 0;
}

} // namespace Secure
