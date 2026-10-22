#include "input/typer.h"

#include <algorithm>

namespace {

LPARAM MakeKeyLParam(UINT vk, bool keyDown) {
    UINT scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC);
    LPARAM lp = 1 | (static_cast<LPARAM>(scan & 0xFF) << 16);
    if (!keyDown) {
        lp |= (1LL << 30);
        lp |= (1LL << 31);
    }
    return lp;
}

void WaitUntilModifiersReleased(int maxWaitMs = 600) {
    const int step = 10;
    for (int elapsed = 0; elapsed < maxWaitMs; elapsed += step) {
        bool down = (GetAsyncKeyState(VK_CONTROL) & 0x8000) ||
                    (GetAsyncKeyState(VK_SHIFT)   & 0x8000) ||
                    (GetAsyncKeyState(VK_MENU)    & 0x8000) ||
                    (GetAsyncKeyState(VK_LWIN)    & 0x8000) ||
                    (GetAsyncKeyState(VK_RWIN)    & 0x8000);
        if (!down) break;
        Sleep(step);
    }
}

void PostKeyTap(HWND hwnd, UINT vk, int holdMs) {
    PostMessageW(hwnd, WM_KEYDOWN, vk, MakeKeyLParam(vk, true));
    if (holdMs > 0) Sleep(holdMs);
    PostMessageW(hwnd, WM_KEYUP, vk, MakeKeyLParam(vk, false));
}

void PostCharUnit(HWND hwnd, wchar_t ch) {
    UINT vk   = LOWORD(VkKeyScanW(ch));
    UINT scan = (vk != 0xFFFF) ? MapVirtualKeyW(vk & 0xFF, MAPVK_VK_TO_VSC) : 0;
    LPARAM lp = 1 | (static_cast<LPARAM>(scan & 0xFF) << 16);
    PostMessageW(hwnd, WM_CHAR, static_cast<WPARAM>(ch), lp);
}

void SendInputKeyTap(UINT vk, int holdMs) {
    UINT scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC);
    INPUT in[2] = {};
    in[0].type       = INPUT_KEYBOARD;
    in[0].ki.wVk     = static_cast<WORD>(vk);
    in[0].ki.wScan   = static_cast<WORD>(scan);
    in[0].ki.dwFlags = 0;
    in[1]            = in[0];
    in[1].ki.dwFlags = KEYEVENTF_KEYUP;

    SendInput(1, &in[0], sizeof(INPUT));
    if (holdMs > 0) Sleep(holdMs);
    SendInput(1, &in[1], sizeof(INPUT));
}

void SendInputCharUnit(wchar_t ch, int holdMs) {
    SHORT vks = VkKeyScanW(ch);
    if (vks != -1) {
        BYTE vk    = LOBYTE(vks);
        BYTE shift = HIBYTE(vks);
        UINT scan  = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC);
        UINT sScan = MapVirtualKeyW(VK_SHIFT, MAPVK_VK_TO_VSC);

        if (shift & 1) {
            INPUT s{};
            s.type     = INPUT_KEYBOARD;
            s.ki.wVk   = VK_SHIFT;
            s.ki.wScan = static_cast<WORD>(sScan);
            SendInput(1, &s, sizeof(INPUT));
            Sleep(5);
        }

        INPUT k[2] = {};
        k[0].type       = INPUT_KEYBOARD;
        k[0].ki.wVk     = vk;
        k[0].ki.wScan   = static_cast<WORD>(scan);
        k[1]            = k[0];
        k[1].ki.dwFlags = KEYEVENTF_KEYUP;
        SendInput(1, &k[0], sizeof(INPUT));
        if (holdMs > 0) Sleep(holdMs);
        SendInput(1, &k[1], sizeof(INPUT));

        if (shift & 1) {
            INPUT s{};
            s.type       = INPUT_KEYBOARD;
            s.ki.wVk     = VK_SHIFT;
            s.ki.wScan   = static_cast<WORD>(sScan);
            s.ki.dwFlags = KEYEVENTF_KEYUP;
            Sleep(5);
            SendInput(1, &s, sizeof(INPUT));
        }
    } else {
        INPUT u[2] = {};
        u[0].type       = INPUT_KEYBOARD;
        u[0].ki.wScan   = static_cast<WORD>(ch);
        u[0].ki.dwFlags = KEYEVENTF_UNICODE;
        u[1]            = u[0];
        u[1].ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;
        SendInput(2, u, sizeof(INPUT));
    }
}

void TapKey(HWND hwnd, InputMethod method, UINT vk, int delayMs, int holdMs = 0) {
    int hold = (holdMs > 0) ? holdMs : std::clamp(delayMs / 2, 5, 25);
    if (method == InputMethod::PostMessage) PostKeyTap(hwnd, vk, hold);
    else                                    SendInputKeyTap(vk, hold);
    if (delayMs > 0) Sleep(delayMs);
}

void TypeBytes(HWND hwnd, InputMethod method, const uint8_t* data, size_t len, int delayMs) {
    if (!data || len == 0) return;
    int wlen = MultiByteToWideChar(CP_UTF8, 0, reinterpret_cast<const char*>(data), (int)len, nullptr, 0);
    if (wlen <= 0) return;

    SecureWide wbuf(static_cast<size_t>(wlen));
    MultiByteToWideChar(CP_UTF8, 0, reinterpret_cast<const char*>(data), (int)len, wbuf.data(), wlen);

    int hold = std::clamp(delayMs / 2, 4, 20);
    for (int i = 0; i < wlen; ++i) {
        if (!IsWindow(hwnd)) break;
        if (method == InputMethod::PostMessage) PostCharUnit(hwnd, wbuf[i]);
        else                                    SendInputCharUnit(wbuf[i], hold);
        if (delayMs > 0) Sleep(delayMs);
    }
}

void ClearField(HWND hwnd, InputMethod method, int clearDelayMs, int holdMs) {
    // Move to the end of the text box and erase up to 24 characters (RO account limit).
    int fastDelay = std::clamp(clearDelayMs, 1, 20);
    TapKey(hwnd, method, VK_END, fastDelay, holdMs);
    for (int i = 0; i < 24; ++i) {
        if (!IsWindow(hwnd)) return;
        if (method == InputMethod::PostMessage) PostKeyTap(hwnd, VK_BACK, 1);
        else                                    SendInputKeyTap(VK_BACK, 1);
        if (fastDelay > 0) Sleep(fastDelay);
    }
    Sleep(10);
}

} // namespace

Typer::~Typer() {
    if (m_worker.joinable()) m_worker.join();
}

bool Typer::Start(TyperJob&& job) {
    bool expected = false;
    if (!m_busy.compare_exchange_strong(expected, true)) {
        SecureWipe(job.password);
        return false;
    }
    if (m_worker.joinable()) m_worker.join();
    m_worker = std::thread(&Typer::Run, std::move(job), &m_busy);
    return true;
}

void Typer::Run(TyperJob job, std::atomic<bool>* busyFlag) {
    struct Guard {
        TyperJob&          j;
        std::atomic<bool>* flag;
        ~Guard() {
            SecureWipe(j.password);
            if (!j.username.empty()) SecureZeroMemory(&j.username[0], j.username.size());
            flag->store(false);
        }
    } guard{ job, busyFlag };

    if (!job.hwnd || !IsWindow(job.hwnd)) return;

    // Wait briefly for the user to let go of the hotkey modifiers (Ctrl/Shift/Alt)
    // so the game client doesn't see Ctrl+char or Shift+Tab.
    WaitUntilModifiersReleased();

    int delay = std::clamp(job.keyDelayMs, 1, 200);
    int hold  = std::clamp(job.keyHoldMs, 1, 100);
    int tabD  = std::clamp(job.tabDelayMs, 1, 500);

    if (job.sequence == LoginSequence::IdTabPassword) {
        // Auto-align: if client is currently focused on the Password field,
        // send Tab first to cycle focus to the User ID field before typing.
        if (job.currentFocus == LoginFieldFocus::Password) {
            TapKey(job.hwnd, job.method, VK_TAB, tabD, hold);
        }
        if (job.clearIdField) ClearField(job.hwnd, job.method, job.clearDelayMs, hold);
        TypeBytes(job.hwnd, job.method,
                  reinterpret_cast<const uint8_t*>(job.username.data()), job.username.size(),
                  delay);
        TapKey(job.hwnd, job.method, VK_TAB, tabD, hold);
    } else if (job.sequence == LoginSequence::PasswordOnly) {
        // Auto-align: if client is currently focused on User ID,
        // send Tab first to cycle focus to the Password field.
        if (job.currentFocus == LoginFieldFocus::UserId) {
            TapKey(job.hwnd, job.method, VK_TAB, tabD, hold);
        }
    }

    TypeBytes(job.hwnd, job.method, job.password.data(), job.password.size(), delay);

    if (job.pressEnter) {
        // Ensure game window is in foreground so dialogs and hardware input are active
        if (job.hwnd && IsWindow(job.hwnd)) {
            SetForegroundWindow(job.hwnd);
        }

        // First Return submits username and password
        Sleep(std::max(delay, 20));
        TapKey(job.hwnd, job.method, VK_RETURN, 20, hold);

        // Pause for the 'Select Service' prompt to appear across the network (default 350ms)
        int pauseMs = std::max(std::clamp(job.enterDelayMs, 20, 1000), 350);
        Sleep(pauseMs);

        // Open client process handle to check when Character Select is reached (UiFlag == 1 at 0x00F33F90)
        HANDLE hProcess = nullptr;
        if (job.targetPid != 0) {
            hProcess = OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION, FALSE, job.targetPid);
            if (!hProcess) hProcess = OpenProcess(PROCESS_VM_READ, FALSE, job.targetPid);
        }

        auto HasReachedCharSelect = [hProcess]() -> bool {
            if (!hProcess) return false;
            uint32_t uiFlag = 0;
            SIZE_T read = 0;
            if (ReadProcessMemory(hProcess, reinterpret_cast<LPCVOID>(0x00F33F90), &uiFlag, 4, &read) && read == 4) {
                return (uiFlag == 1);
            }
            return false;
        };

        // If client somehow reached Character Select already, do not send Return
        if (!HasReachedCharSelect()) {
            const int maxAttempts = (hProcess != nullptr) ? 3 : 1;
            // Allow 600ms between attempts: Char Server connection takes ~250-400ms.
            // 600ms gives ample time for UiFlag to flip to 1 before another attempt can fire,
            // strictly guaranteeing that we never send an extra Return into Character Select.
            const int pollWindowMs = 600;

            for (int attempt = 0; attempt < maxAttempts; ++attempt) {
                if (!IsWindow(job.hwnd)) break;

                // Bring to foreground if focus shifted
                if (GetForegroundWindow() != job.hwnd) {
                    SetForegroundWindow(job.hwnd);
                }

                // Send exactly ONE hardware Return to confirm 'Select Service'
                if (GetForegroundWindow() == job.hwnd || job.method == InputMethod::SendInput) {
                    SendInputKeyTap(VK_RETURN, hold);
                } else {
                    PostKeyTap(job.hwnd, VK_RETURN, hold);
                }

                // Poll UiFlag every 25ms over the 600ms window
                bool reached = false;
                for (int elapsed = 0; elapsed < pollWindowMs; elapsed += 25) {
                    Sleep(25);
                    if (HasReachedCharSelect()) {
                        reached = true;
                        break;
                    }
                }
                // The instant Character Select is confirmed, STOP! Never send another Return.
                if (reached) break;
            }
        }

        if (hProcess) CloseHandle(hProcess);
    }
}
