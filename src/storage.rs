use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Returns the directory containing the running executable.
pub fn get_exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Returns whether ORLogin is operating in portable mode.
pub fn is_portable_mode() -> bool {
    if cfg!(feature = "portable") {
        return true;
    }
    get_exe_dir().join("portable.flag").exists()
}

/// Returns the active storage directory for the vault and settings:
/// - In portable mode (`portable` feature or `portable.flag` beside the exe), returns `get_exe_dir()`.
/// - Otherwise returns `%LOCALAPPDATA%\ORLogin`, creating it if needed and migrating
///   existing `ORLogin.vault` / `ORLogin.vault.bak` / `ORLogin.settings.json` from the exe dir.
pub fn get_storage_dir() -> PathBuf {
    let exe_dir = get_exe_dir();
    if is_portable_mode() {
        return exe_dir;
    }

    let Ok(local_app_data) = std::env::var("LOCALAPPDATA") else {
        return exe_dir;
    };
    if local_app_data.is_empty() {
        return exe_dir;
    }

    let app_data_dir = PathBuf::from(local_app_data).join("ORLogin");
    let _ = fs::create_dir_all(&app_data_dir);

    // Automatic migration from v0.4 portable layout if vault exists beside the exe
    // and has not yet been migrated to %LOCALAPPDATA%\ORLogin.
    let old_vault = exe_dir.join("ORLogin.vault");
    let new_vault = app_data_dir.join("ORLogin.vault");
    if old_vault.is_file() && !new_vault.exists() {
        let _ = fs::copy(&old_vault, &new_vault);

        let old_bak = exe_dir.join("ORLogin.vault.bak");
        let new_bak = app_data_dir.join("ORLogin.vault.bak");
        if old_bak.is_file() && !new_bak.exists() {
            let _ = fs::copy(&old_bak, &new_bak);
        }

        let old_settings = exe_dir.join("ORLogin.settings.json");
        let new_settings = app_data_dir.join("ORLogin.settings.json");
        if old_settings.is_file() && !new_settings.exists() {
            let _ = fs::copy(&old_settings, &new_settings);
        }
    }

    app_data_dir
}

pub fn read_text_file(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

pub fn write_text_file_atomic(path: &Path, content: &str) -> io::Result<()> {
    write_bytes_atomic(path, content.as_bytes(), false)
}

/// Atomically writes `data` via `<path>.tmp` (`sync_all` + `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`).
/// When `keep_backup` is true and `path` already exists, copies the previous file to `<path>.bak` first.
pub fn write_bytes_atomic(path: &Path, data: &[u8], keep_backup: bool) -> io::Result<()> {
    use std::io::Write;

    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp_path = PathBuf::from(tmp);

    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(data)?;
        f.sync_all()?;
    }

    if keep_backup && path.is_file() {
        let mut bak = path.as_os_str().to_os_string();
        bak.push(".bak");
        let _ = fs::copy(path, PathBuf::from(bak));
    }

    if let Err(e) = replace_file_write_through(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }
    Ok(())
}

#[cfg(windows)]
fn replace_file_write_through(src: &Path, dst: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let src_w: Vec<u16> = src.as_os_str().encode_wide().chain(Some(0)).collect();
    let dst_w: Vec<u16> = dst.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        MoveFileExW(
            PCWSTR(src_w.as_ptr()),
            PCWSTR(dst_w.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|e| io::Error::from_raw_os_error(e.code().0))
    }
}

#[cfg(not(windows))]
fn replace_file_write_through(src: &Path, dst: &Path) -> io::Result<()> {
    fs::rename(src, dst)
}
