use crate::config::{InputMethod, LoginSequence};
use crate::game::monitor::LoginFieldFocus;
use crate::game::reader::ProcessReader;
use crate::security::secure::SecureBytes;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, VkKeyScanW, INPUT, INPUT_0, INPUT_KEYBOARD,
    KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_BACK,
    VK_CONTROL, VK_END, VK_LWIN, VK_MENU, VK_RETURN, VK_RWIN, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IsWindow, PostMessageW, SetForegroundWindow, WM_CHAR, WM_KEYDOWN,
    WM_KEYUP,
};
use zeroize::{Zeroize, ZeroizeOnDrop};

const CHAR_SELECT_UI_FLAG_ADDR: u32 = 0x00F3_3F90;

#[derive(Zeroize, ZeroizeOnDrop)]
struct ZeroizingWide(Vec<u16>);

pub struct TyperJob {
    pub hwnd_raw: isize,
    pub target_pid: u32,
    pub expected_exe: &'static str,
    pub username: String,
    pub password: SecureBytes,
    pub sequence: LoginSequence,
    pub current_focus: LoginFieldFocus,
    pub clear_id_field: bool,
    pub press_enter: bool,
    pub method: InputMethod,
    pub key_delay_ms: i32,
    pub enter_delay_ms: i32,
    pub tab_delay_ms: i32,
    pub clear_delay_ms: i32,
    pub key_hold_ms: i32,
}

impl Drop for TyperJob {
    fn drop(&mut self) {
        self.username.zeroize();
        self.password.zeroize();
    }
}

pub struct Typer {
    busy: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Default for Typer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Typer {
    fn drop(&mut self) {
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl Typer {
    pub fn new() -> Self {
        Self {
            busy: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    /// Starts typing `job` asynchronously. Returns `false` if another job is active.
    pub fn start(&mut self, job: TyperJob) -> bool {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            drop(job);
            return false;
        }
        if let Some(prev) = self.worker.take() {
            let _ = prev.join();
        }
        let busy = Arc::clone(&self.busy);
        self.worker = Some(thread::spawn(move || {
            struct BusyGuard(Arc<AtomicBool>);
            impl Drop for BusyGuard {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::Release);
                }
            }
            let _busy_guard = BusyGuard(busy);
            run_job(job);
        }));
        true
    }
}

fn verify_target_process_image(pid: u32, expected_exe: &str) -> bool {
    if pid == 0 {
        return true;
    }
    unsafe {
        let Ok(h_proc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return true;
        };
        if h_proc.is_invalid() {
            return true;
        }
        let mut buf = [0u16; 260];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            h_proc,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(h_proc);

        if ok && size > 0 {
            let full = String::from_utf16_lossy(&buf[..size as usize]);
            if let Some(fname) = Path::new(&full).file_name().and_then(|s| s.to_str()) {
                return fname.eq_ignore_ascii_case(expected_exe);
            }
        }
    }
    true
}

fn wait_until_modifiers_released(max_wait_ms: u32) {
    let step = 10u32;
    let mut elapsed = 0u32;
    while elapsed < max_wait_ms {
        let down = unsafe {
            (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000) != 0
        };
        if !down {
            break;
        }
        thread::sleep(Duration::from_millis(step as u64));
        elapsed += step;
    }
}

fn make_key_lparam(vk: u16, key_down: bool) -> LPARAM {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) };
    let mut lp: isize = 1 | (((scan & 0xFF) as isize) << 16);
    if !key_down {
        lp |= 1isize << 30;
        lp |= 1isize << 31;
    }
    LPARAM(lp)
}

fn post_key_tap(hwnd: HWND, vk: u16, hold_ms: i32) {
    unsafe {
        let _ = PostMessageW(hwnd, WM_KEYDOWN, WPARAM(vk as usize), make_key_lparam(vk, true));
        if hold_ms > 0 {
            thread::sleep(Duration::from_millis(hold_ms as u64));
        }
        let _ = PostMessageW(hwnd, WM_KEYUP, WPARAM(vk as usize), make_key_lparam(vk, false));
    }
}

fn post_char_unit(hwnd: HWND, ch: u16) {
    unsafe {
        let vk_scan = VkKeyScanW(ch);
        let scan = if vk_scan != -1 {
            MapVirtualKeyW((vk_scan as u16 & 0xFF) as u32, MAPVK_VK_TO_VSC)
        } else {
            0
        };
        let lp = LPARAM(1 | (((scan & 0xFF) as isize) << 16));
        let _ = PostMessageW(hwnd, WM_CHAR, WPARAM(ch as usize), lp);
    }
}

fn send_input_key_tap(vk: u16, hold_ms: i32) {
    unsafe {
        let scan = MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u16;
        let down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: scan,
                    ..Default::default()
                },
            },
        };
        let mut up = down;
        up.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

        let _ = SendInput(&[down], std::mem::size_of::<INPUT>() as i32);
        if hold_ms > 0 {
            thread::sleep(Duration::from_millis(hold_ms as u64));
        }
        let _ = SendInput(&[up], std::mem::size_of::<INPUT>() as i32);
    }
}

fn send_input_char_unit(ch: u16, hold_ms: i32) {
    unsafe {
        let vks = VkKeyScanW(ch);
        if vks != -1 {
            let vk = (vks as u16) & 0xFF;
            let shift = ((vks as u16) >> 8) & 0xFF;
            let scan = MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u16;
            let shift_scan = MapVirtualKeyW(VK_SHIFT.0 as u32, MAPVK_VK_TO_VSC) as u16;

            if (shift & 1) != 0 {
                let s_down = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_SHIFT,
                            wScan: shift_scan,
                            ..Default::default()
                        },
                    },
                };
                let _ = SendInput(&[s_down], std::mem::size_of::<INPUT>() as i32);
                thread::sleep(Duration::from_millis(5));
            }

            let down = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(vk),
                        wScan: scan,
                        ..Default::default()
                    },
                },
            };
            let mut up = down;
            up.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            let _ = SendInput(&[down], std::mem::size_of::<INPUT>() as i32);
            if hold_ms > 0 {
                thread::sleep(Duration::from_millis(hold_ms as u64));
            }
            let _ = SendInput(&[up], std::mem::size_of::<INPUT>() as i32);

            if (shift & 1) != 0 {
                let s_up = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_SHIFT,
                            wScan: shift_scan,
                            dwFlags: KEYEVENTF_KEYUP,
                            ..Default::default()
                        },
                    },
                };
                thread::sleep(Duration::from_millis(5));
                let _ = SendInput(&[s_up], std::mem::size_of::<INPUT>() as i32);
            }
        } else {
            let down = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wScan: ch,
                        dwFlags: KEYEVENTF_UNICODE,
                        ..Default::default()
                    },
                },
            };
            let mut up = down;
            up.Anonymous.ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;
            let _ = SendInput(&[down, up], std::mem::size_of::<INPUT>() as i32);
        }
    }
}

fn tap_key(hwnd: HWND, method: InputMethod, vk: u16, delay_ms: i32, hold_ms: i32) -> bool {
    unsafe {
        if !IsWindow(hwnd).as_bool() {
            return false;
        }
        let hold = if hold_ms > 0 {
            hold_ms
        } else {
            (delay_ms / 2).clamp(5, 25)
        };
        match method {
            InputMethod::SendInput => {
                if GetForegroundWindow() != hwnd {
                    return false;
                }
                send_input_key_tap(vk, hold);
            }
            InputMethod::PostMessage => post_key_tap(hwnd, vk, hold),
        }
    }
    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms as u64));
    }
    true
}

fn type_utf8_bytes(hwnd: HWND, method: InputMethod, bytes: &[u8], delay_ms: i32) -> bool {
    if bytes.is_empty() {
        return true;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let wbuf = ZeroizingWide(text.encode_utf16().collect());
    let hold = (delay_ms / 2).clamp(4, 20);

    for &ch in &wbuf.0 {
        unsafe {
            if !IsWindow(hwnd).as_bool() {
                return false;
            }
            match method {
                InputMethod::SendInput => {
                    if GetForegroundWindow() != hwnd {
                        // Focus shifted away; abort immediately to prevent credential leakage.
                        return false;
                    }
                    send_input_char_unit(ch, hold);
                }
                InputMethod::PostMessage => post_char_unit(hwnd, ch),
            }
        }
        if delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms as u64));
        }
    }
    true
}

fn clear_field(hwnd: HWND, method: InputMethod, clear_delay_ms: i32, hold_ms: i32) -> bool {
    let fast_delay = clear_delay_ms.clamp(1, 20);
    if !tap_key(hwnd, method, VK_END.0, fast_delay, hold_ms) {
        return false;
    }
    for _ in 0..24 {
        unsafe {
            if !IsWindow(hwnd).as_bool() {
                return false;
            }
            match method {
                InputMethod::SendInput => {
                    if GetForegroundWindow() != hwnd {
                        return false;
                    }
                    send_input_key_tap(VK_BACK.0, 1);
                }
                InputMethod::PostMessage => post_key_tap(hwnd, VK_BACK.0, 1),
            }
        }
        if fast_delay > 0 {
            thread::sleep(Duration::from_millis(fast_delay as u64));
        }
    }
    thread::sleep(Duration::from_millis(10));
    true
}

fn run_job(job: TyperJob) {
    if job.hwnd_raw == 0 {
        return;
    }
    let hwnd = HWND(job.hwnd_raw as *mut _);
    if !unsafe { IsWindow(hwnd).as_bool() } {
        return;
    }
    if !verify_target_process_image(job.target_pid, job.expected_exe) {
        return;
    }

    wait_until_modifiers_released(600);

    if job.method == InputMethod::SendInput {
        unsafe {
            let _ = SetForegroundWindow(hwnd);
        }
        thread::sleep(Duration::from_millis(30));
        if unsafe { GetForegroundWindow() != hwnd } {
            return;
        }
    }

    let delay = job.key_delay_ms.clamp(1, 200);
    let hold = job.key_hold_ms.clamp(1, 100);
    let tab_d = job.tab_delay_ms.clamp(1, 500);

    match job.sequence {
        LoginSequence::IdTabPassword => {
            if job.current_focus == LoginFieldFocus::Password
                && !tap_key(hwnd, job.method, VK_TAB.0, tab_d, hold)
            {
                return;
            }
            if job.clear_id_field && !clear_field(hwnd, job.method, job.clear_delay_ms, hold) {
                return;
            }
            if !type_utf8_bytes(hwnd, job.method, job.username.as_bytes(), delay) {
                return;
            }
            if !tap_key(hwnd, job.method, VK_TAB.0, tab_d, hold) {
                return;
            }
        }
        LoginSequence::PasswordOnly => {
            if job.current_focus == LoginFieldFocus::UserId
                && !tap_key(hwnd, job.method, VK_TAB.0, tab_d, hold)
            {
                return;
            }
        }
    }

    if !type_utf8_bytes(hwnd, job.method, job.password.as_slice(), delay) {
        return;
    }

    if job.press_enter {
        unsafe {
            if IsWindow(hwnd).as_bool() {
                let _ = SetForegroundWindow(hwnd);
            }
        }
        let reader = if job.target_pid != 0 {
            ProcessReader::open(job.target_pid)
        } else {
            None
        };
        let is_at_char_select = || -> bool {
            reader
                .as_ref()
                .and_then(|r| r.read_u32(CHAR_SELECT_UI_FLAG_ADDR))
                .is_some_and(|v| v == 1)
        };

        thread::sleep(Duration::from_millis(delay.max(20) as u64));
        if !tap_key(hwnd, job.method, VK_RETURN.0, 20, hold) {
            return;
        }
        if is_at_char_select() {
            return;
        }

        let pause_ms = job.enter_delay_ms.clamp(50, 1000);
        thread::sleep(Duration::from_millis(pause_ms as u64));
        if is_at_char_select() {
            return;
        }

        unsafe {
            if GetForegroundWindow() != hwnd {
                let _ = SetForegroundWindow(hwnd);
            }
            if GetForegroundWindow() == hwnd {
                send_input_key_tap(VK_RETURN.0, hold);
            } else if job.method == InputMethod::PostMessage {
                post_key_tap(hwnd, VK_RETURN.0, hold);
            }
        }
    }
}
