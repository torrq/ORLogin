#pragma once

// Asynchronous credential typer for OSRO clients.
// Runs on a short-lived worker thread so key delays never block the UI render loop.
// Plaintext password lives in a locked SecureBytes inside the job and is wiped as
// soon as typing finishes.

#include <windows.h>
#include <atomic>
#include <string>
#include <thread>

#include "security/secure.h"
#include "settings.h"

struct TyperJob {
    HWND          hwnd         = nullptr;
    std::string   username;
    SecureBytes   password;    // wiped immediately when the worker exits
    LoginSequence sequence     = LoginSequence::IdTabPassword;
    bool          clearIdField = true;
    bool          pressEnter   = true;
    InputMethod   method       = InputMethod::PostMessage;
    int           keyDelayMs   = 15;
};

class Typer {
public:
    Typer() = default;
    ~Typer();

    Typer(const Typer&) = delete;
    Typer& operator=(const Typer&) = delete;

    // Starts typing `job` asynchronously. Returns false if already busy.
    bool Start(TyperJob&& job);

    bool IsBusy() const { return m_busy.load(); }

private:
    static void Run(TyperJob job, std::atomic<bool>* busyFlag);

    std::thread       m_worker;
    std::atomic<bool> m_busy{ false };
};
