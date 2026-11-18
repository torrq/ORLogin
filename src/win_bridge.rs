use crate::config::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};
use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use windows::core::w;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyNameTextW, MapVirtualKeyW, RegisterHotKey, UnregisterHotKey,
    HOT_KEY_MODIFIERS, MAPVK_VK_TO_VSC, MOD_NOREPEAT, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN,
    VK_END, VK_ESCAPE, VK_F1, VK_F24, VK_HOME, VK_INSERT, VK_LBUTTON, VK_LCONTROL, VK_LEFT,
    VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MBUTTON, VK_MENU, VK_NEXT, VK_NUMPAD0, VK_NUMPAD9, VK_OEM_3,
    VK_OEM_MINUS, VK_OEM_PLUS, VK_PRIOR, VK_RBUTTON, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP, VK_XBUTTON1, VK_XBUTTON2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows,
    GetForegroundWindow, GetMessageW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, PostMessageW, PostQuitMessage, RegisterClassExW,
    SetForegroundWindow, SetWindowDisplayAffinity, ShowWindow, TranslateMessage, CREATESTRUCTW,
    GWLP_USERDATA, HWND_MESSAGE, MSG, SW_RESTORE, WINDOW_DISPLAY_AFFINITY, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_APP, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_HOTKEY, WM_WTSSESSION_CHANGE,
    WNDCLASSEXW, WTS_SESSION_LOCK,
};

// `WDA_NONE = 0x0`, `WDA_EXCLUDEFROMCAPTURE = 0x11`
const WDA_NONE_VAL: u32 = 0x0000_0000;
const WDA_EXCLUDEFROMCAPTURE_VAL: u32 = 0x0000_0011;

const HOTKEY_ID: i32 = 1001;
const WM_APP_SET_HOTKEY: u32 = WM_APP + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinEvent {
    HotkeyPressed,
    SessionLocked,
    HotkeyStatus(bool),
}

struct BridgeState {
    tx: Sender<WinEvent>,
    ctx: egui::Context,
}

/// Owns a hidden `HWND_MESSAGE` Win32 loop on a background thread for global hotkey
/// (`WM_HOTKEY`) and workstation lock (`WM_WTSSESSION_CHANGE`) delivery without subclassing `winit`.
pub struct WinBridge {
    hwnd_raw: Arc<Mutex<isize>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for WinBridge {
    fn drop(&mut self) {
        let raw = *self.hwnd_raw.lock().unwrap();
        if raw != 0 {
            unsafe {
                let _ = PostMessageW(HWND(raw as *mut _), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }
}

impl WinBridge {
    pub fn spawn(
        ctx: egui::Context,
        initial_mods: u32,
        initial_vk: u32,
    ) -> (Self, Receiver<WinEvent>) {
        let (tx, rx) = unbounded();
        let hwnd_raw = Arc::new(Mutex::new(0isize));
        let hwnd_raw_clone = Arc::clone(&hwnd_raw);

        let (ready_tx, ready_rx) = crossbeam_channel::bounded::<isize>(1);

        let thread = thread::spawn(move || {
            run_message_loop(tx, ctx, ready_tx, hwnd_raw_clone, initial_mods, initial_vk);
        });

        let _ = ready_rx.recv();
        (Self { hwnd_raw, thread: Some(thread) }, rx)
    }

    pub fn set_hotkey(&self, mods: u32, vk: u32) {
        let raw = *self.hwnd_raw.lock().unwrap();
        if raw != 0 {
            unsafe {
                let _ = PostMessageW(
                    HWND(raw as *mut _),
                    WM_APP_SET_HOTKEY,
                    WPARAM(mods as usize),
                    LPARAM(vk as isize),
                );
            }
        }
    }
}

fn run_message_loop(
    tx: Sender<WinEvent>,
    ctx: egui::Context,
    ready_tx: Sender<isize>,
    hwnd_slot: Arc<Mutex<isize>>,
    initial_mods: u32,
    initial_vk: u32,
) {
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("ORLoginMessageBridgeClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(bridge_wnd_proc),
            hInstance: hinst.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        let boxed = Box::new(BridgeState { tx, ctx });
        let state_ptr = Box::into_raw(boxed);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("ORLoginBridge"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            hinst,
            Some(state_ptr as *const _),
        )
        .unwrap_or_default();

        if hwnd.0.is_null() {
            let _ = Box::from_raw(state_ptr);
            let _ = ready_tx.send(0);
            return;
        }

        *hwnd_slot.lock().unwrap() = hwnd.0 as isize;
        let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);

        // Register initial global hotkey
        let _ = PostMessageW(
            hwnd,
            WM_APP_SET_HOTKEY,
            WPARAM(initial_mods as usize),
            LPARAM(initial_vk as isize),
        );
        let _ = ready_tx.send(hwnd.0 as isize);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        *hwnd_slot.lock().unwrap() = 0;
        let _ = Box::from_raw(state_ptr);
    }
}

unsafe extern "system" fn bridge_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        let cs = &*(lparam.0 as *const CREATESTRUCTW);
        windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            GWLP_USERDATA,
            cs.lpCreateParams as isize,
        );
        return LRESULT(0);
    }

    let ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA)
        as *mut BridgeState;
    if !ptr.is_null() {
        let state = &*ptr;
        match msg {
            WM_HOTKEY if wparam.0 as i32 == HOTKEY_ID => {
                let _ = state.tx.send(WinEvent::HotkeyPressed);
                state.ctx.request_repaint();
                return LRESULT(0);
            }
            WM_WTSSESSION_CHANGE if wparam.0 as u32 == WTS_SESSION_LOCK => {
                let _ = state.tx.send(WinEvent::SessionLocked);
                state.ctx.request_repaint();
                return LRESULT(0);
            }
            WM_APP_SET_HOTKEY => {
                let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
                let mods = wparam.0 as u32;
                let vk = lparam.0 as u32;
                let ok = if vk != 0 {
                    RegisterHotKey(hwnd, HOTKEY_ID, HOT_KEY_MODIFIERS(mods | MOD_NOREPEAT.0), vk)
                        .is_ok()
                } else {
                    false
                };
                let _ = state.tx.send(WinEvent::HotkeyStatus(ok));
                state.ctx.request_repaint();
                return LRESULT(0);
            }
            WM_CLOSE => {
                let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
                let _ = WTSUnRegisterSessionNotification(hwnd);
                let _ = DestroyWindow(hwnd);
                return LRESULT(0);
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                return LRESULT(0);
            }
            _ => {}
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

pub fn get_foreground_hwnd_raw() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

pub fn get_foreground_pid() -> u32 {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return 0;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut pid));
        pid
    }
}

pub fn set_foreground_window(hwnd_raw: isize) {
    if hwnd_raw != 0 {
        unsafe {
            let hwnd = HWND(hwnd_raw as *mut _);
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyCapture {
    Waiting,
    Cancelled,
    Captured { mods: u32, vk: u32 },
}

pub fn poll_hotkey_capture() -> HotkeyCapture {
    unsafe {
        if (GetAsyncKeyState(VK_ESCAPE.0 as i32) as u16 & 0x8000) != 0 {
            return HotkeyCapture::Cancelled;
        }
        let mut mods = 0u32;
        if (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0 {
            mods |= MOD_CONTROL;
        }
        if (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0 {
            mods |= MOD_SHIFT;
        }
        if (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0 {
            mods |= MOD_ALT;
        }
        if ((GetAsyncKeyState(VK_LWIN.0 as i32) | GetAsyncKeyState(VK_RWIN.0 as i32)) as u16
            & 0x8000)
            != 0
        {
            mods |= MOD_WIN;
        }

        for vk in 0x08u32..=0xFEu32 {
            let v = vk as u16;
            if v == VK_LBUTTON.0
                || v == VK_RBUTTON.0
                || v == VK_MBUTTON.0
                || v == VK_XBUTTON1.0
                || v == VK_XBUTTON2.0
                || v == VK_ESCAPE.0
                || is_modifier_vk(vk)
            {
                continue;
            }
            if (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 {
                return HotkeyCapture::Captured { mods, vk };
            }
        }
    }
    HotkeyCapture::Waiting
}

pub fn set_window_display_affinity(hwnd_raw: isize, exclude_from_capture: bool) {
    if hwnd_raw == 0 {
        return;
    }
    let val = if exclude_from_capture {
        WDA_EXCLUDEFROMCAPTURE_VAL
    } else {
        WDA_NONE_VAL
    };
    unsafe {
        let _ = SetWindowDisplayAffinity(HWND(hwnd_raw as *mut _), WINDOW_DISPLAY_AFFINITY(val));
    }
}

pub fn apply_titlebar_dark_mode(hwnd_raw: isize, dark: bool) {
    if hwnd_raw == 0 {
        return;
    }
    let use_dark: BOOL = if dark { BOOL(1) } else { BOOL(0) };
    unsafe {
        let _ = DwmSetWindowAttribute(
            HWND(hwnd_raw as *mut _),
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &use_dark as *const BOOL as *const _,
            std::mem::size_of::<BOOL>() as u32,
        );
    }
}

/// Restores & foregrounds an existing `ORLogin` window matching title prefix `"OSRO Login Helper"`.
pub fn activate_existing_window_by_prefix(prefix: &str) {
    struct Ctx<'a> {
        prefix: &'a str,
    }
    unsafe extern "system" fn cb(hwnd: HWND, lp: LPARAM) -> BOOL {
        let ctx = &*(lp.0 as *const Ctx<'_>);
        let len = GetWindowTextLengthW(hwnd);
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            let read = GetWindowTextW(hwnd, &mut buf);
            if read > 0 {
                let title = String::from_utf16_lossy(&buf[..read as usize]);
                if title.starts_with(ctx.prefix) {
                    if IsIconic(hwnd).as_bool() {
                        let _ = ShowWindow(hwnd, SW_RESTORE);
                    }
                    let _ = SetForegroundWindow(hwnd);
                    return BOOL(0);
                }
            }
        }
        BOOL(1)
    }
    let ctx = Ctx { prefix };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&ctx as *const _ as isize));
    }
}

pub fn is_modifier_vk(vk: u32) -> bool {
    matches!(
        vk as u16,
        x if x == VK_SHIFT.0
            || x == VK_LSHIFT.0
            || x == VK_RSHIFT.0
            || x == VK_CONTROL.0
            || x == VK_LCONTROL.0
            || x == VK_RCONTROL.0
            || x == VK_MENU.0
            || x == VK_LMENU.0
            || x == VK_RMENU.0
            || x == VK_LWIN.0
            || x == VK_RWIN.0
    )
}

pub fn key_name(vk: u32) -> String {
    if (b'A' as u32..=b'Z' as u32).contains(&vk) || (b'0' as u32..=b'9' as u32).contains(&vk) {
        return (vk as u8 as char).to_string();
    }
    if (VK_F1.0 as u32..=VK_F24.0 as u32).contains(&vk) {
        return format!("F{}", vk - VK_F1.0 as u32 + 1);
    }
    if (VK_NUMPAD0.0 as u32..=VK_NUMPAD9.0 as u32).contains(&vk) {
        return format!("Num{}", vk - VK_NUMPAD0.0 as u32);
    }

    match vk as u16 {
        x if x == VK_SPACE.0 => return "Space".to_owned(),
        x if x == VK_TAB.0 => return "Tab".to_owned(),
        x if x == VK_RETURN.0 => return "Enter".to_owned(),
        x if x == VK_BACK.0 => return "Backspace".to_owned(),
        x if x == VK_INSERT.0 => return "Insert".to_owned(),
        x if x == VK_DELETE.0 => return "Delete".to_owned(),
        x if x == VK_HOME.0 => return "Home".to_owned(),
        x if x == VK_END.0 => return "End".to_owned(),
        x if x == VK_PRIOR.0 => return "PageUp".to_owned(),
        x if x == VK_NEXT.0 => return "PageDown".to_owned(),
        x if x == VK_UP.0 => return "Up".to_owned(),
        x if x == VK_DOWN.0 => return "Down".to_owned(),
        x if x == VK_LEFT.0 => return "Left".to_owned(),
        x if x == VK_RIGHT.0 => return "Right".to_owned(),
        x if x == VK_OEM_3.0 => return "`".to_owned(),
        x if x == VK_OEM_MINUS.0 => return "-".to_owned(),
        x if x == VK_OEM_PLUS.0 => return "=".to_owned(),
        _ => {}
    }

    unsafe {
        let scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC);
        if scan != 0 {
            let mut buf = [0u16; 64];
            let len = GetKeyNameTextW((scan << 16) as i32, &mut buf);
            if len > 0 {
                return String::from_utf16_lossy(&buf[..len as usize]);
            }
        }
    }
    format!("0x{vk:02X}")
}

pub fn format_hotkey(mods: u32, vk: u32) -> String {
    if vk == 0 {
        return "None".to_owned();
    }
    let mut out = String::new();
    if (mods & MOD_CONTROL) != 0 {
        out.push_str("Ctrl + ");
    }
    if (mods & MOD_SHIFT) != 0 {
        out.push_str("Shift + ");
    }
    if (mods & MOD_ALT) != 0 {
        out.push_str("Alt + ");
    }
    if (mods & MOD_WIN) != 0 {
        out.push_str("Win + ");
    }
    out.push_str(&key_name(vk));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_formatting_matches_legacy() {
        assert_eq!(format_hotkey(MOD_CONTROL | MOD_SHIFT, 'L' as u32), "Ctrl + Shift + L");
        assert_eq!(format_hotkey(MOD_ALT, VK_F5_TEST), "Alt + F5");
        assert_eq!(format_hotkey(0, 0), "None");
    }

    const VK_F5_TEST: u32 = 0x74;
}
