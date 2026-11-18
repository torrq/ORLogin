#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use orlogin::app::ORLoginApp;
use orlogin::config;
use orlogin::security::secure;
use orlogin::storage;
use orlogin::theme;
use orlogin::win_bridge;
use std::ffi::c_void;
use std::sync::Arc;
use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::LibraryLoader::{
    GetModuleHandleW, GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use windows::Win32::System::Threading::CreateMutexW;

const BASE_SEARCH_PATH_ENABLE_SAFE_SEARCHMODE: u32 = 0x0000_0001;
const BASE_SEARCH_PATH_PERMANENT: u32 = 0x0000_8000;

struct MutexGuard(HANDLE);
impl Drop for MutexGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Hardens DLL search order at process startup via dynamic `GetProcAddress` lookup
/// (preserving legacy Windows 7 compatibility without hard import failure).
fn apply_dll_search_hardening() {
    unsafe {
        let Ok(h_k32) = GetModuleHandleW(w!("kernel32.dll")) else {
            return;
        };
        if let Some(p_fn) = GetProcAddress(h_k32, windows::core::s!("SetDefaultDllDirectories")) {
            let f: unsafe extern "system" fn(u32) -> i32 = std::mem::transmute(p_fn);
            f(LOAD_LIBRARY_SEARCH_SYSTEM32.0);
        }
        if let Some(p_fn) = GetProcAddress(h_k32, windows::core::s!("SetSearchPathMode")) {
            let f: unsafe extern "system" fn(u32) -> i32 = std::mem::transmute(p_fn);
            f(BASE_SEARCH_PATH_ENABLE_SAFE_SEARCHMODE | BASE_SEARCH_PATH_PERMANENT);
        }
    }
}

fn acquire_single_instance_mutex() -> Option<MutexGuard> {
    unsafe {
        let Ok(h_mutex) = CreateMutexW(None, true, w!("Local\\ORLogin_SingleInstance_Mutex_B6D83D71"))
        else {
            return None;
        };
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(h_mutex);
            win_bridge::activate_existing_window_by_prefix("OSRO Login Helper");
            return None;
        }
        let _: *mut c_void = h_mutex.0;
        Some(MutexGuard(h_mutex))
    }
}

fn main() -> eframe::Result<()> {
    apply_dll_search_hardening();
    secure::init_process_memory();

    let Some(_instance_guard) = acquire_single_instance_mutex() else {
        return Ok(());
    };

    let window_title = "OSRO Login Helper v6";
    let storage_dir = storage::get_storage_dir();
    let settings = config::load_settings(&storage_dir.join("ORLogin.settings.json"));

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(window_title)
        .with_inner_size([880.0, 720.0])
        .with_min_inner_size([720.0, 560.0]);
    if let Some(icon) = theme::rasterize_app_icon_svg(include_bytes!("../assets/icon.svg"), 256) {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "ORLogin",
        native_options,
        Box::new(move |cc| Ok(Box::new(ORLoginApp::new(cc, storage_dir, settings)))),
    )
}
