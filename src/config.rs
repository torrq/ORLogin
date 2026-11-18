use crate::storage;
use serde::{Deserialize, Serialize};
use std::path::Path;

// Win32 modifier / virtual-key constants matching C++ settings.h
pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;
const MOD_MASK: u32 = MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LoginSequence {
    IdTabPassword = 0,
    PasswordOnly = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum InputMethod {
    PostMessage = 0,
    SendInput = 1,
}

/// Security-critical settings stored encrypted and authenticated inside the Vault payload (v2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultSettings {
    pub sequence: LoginSequence,
    pub clear_id_field: bool,
    pub press_enter: bool,
    pub input_method: InputMethod,
    pub key_delay_ms: i32,
    pub enter_delay_ms: i32,
    pub tab_delay_ms: i32,
    pub clear_delay_ms: i32,
    pub key_hold_ms: i32,
    pub auto_lock_minutes: i32,
    pub require_login_screen: bool,
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            sequence: LoginSequence::IdTabPassword,
            clear_id_field: true,
            press_enter: true,
            input_method: InputMethod::PostMessage,
            key_delay_ms: 15,
            enter_delay_ms: 300,
            tab_delay_ms: 25,
            clear_delay_ms: 3,
            key_hold_ms: 10,
            auto_lock_minutes: 15,
            require_login_screen: true,
        }
    }
}

impl VaultSettings {
    pub fn clamp(&mut self) {
        self.key_delay_ms = self.key_delay_ms.clamp(1, 200);
        self.enter_delay_ms = self.enter_delay_ms.clamp(20, 1000);
        self.tab_delay_ms = self.tab_delay_ms.clamp(5, 500);
        self.clear_delay_ms = self.clear_delay_ms.clamp(1, 50);
        self.key_hold_ms = self.key_hold_ms.clamp(2, 100);
        self.auto_lock_minutes = self.auto_lock_minutes.clamp(0, 240);
    }
}

/// Application-wide settings combining cosmetic UI JSON preferences and vault-backed settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    // Cosmetic / startup preferences (stored in plaintext ORLogin.settings.json)
    pub dark_theme: bool,
    pub hotkey_vk: u32,
    pub hotkey_mods: u32,
    pub hide_from_capture: bool,

    // Security-critical preferences (synchronized with encrypted vault payload v2)
    pub sequence: LoginSequence,
    pub clear_id_field: bool,
    pub press_enter: bool,
    pub input_method: InputMethod,
    pub key_delay_ms: i32,
    pub enter_delay_ms: i32,
    pub tab_delay_ms: i32,
    pub clear_delay_ms: i32,
    pub key_hold_ms: i32,
    pub auto_lock_minutes: i32,
    pub require_login_screen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let vs = VaultSettings::default();
        Self {
            dark_theme: true,
            hotkey_vk: 'L' as u32,
            hotkey_mods: MOD_CONTROL | MOD_SHIFT,
            hide_from_capture: false,
            sequence: vs.sequence,
            clear_id_field: vs.clear_id_field,
            press_enter: vs.press_enter,
            input_method: vs.input_method,
            key_delay_ms: vs.key_delay_ms,
            enter_delay_ms: vs.enter_delay_ms,
            tab_delay_ms: vs.tab_delay_ms,
            clear_delay_ms: vs.clear_delay_ms,
            key_hold_ms: vs.key_hold_ms,
            auto_lock_minutes: vs.auto_lock_minutes,
            require_login_screen: vs.require_login_screen,
        }
    }
}

impl Settings {
    pub fn to_vault_settings(&self) -> VaultSettings {
        VaultSettings {
            sequence: self.sequence,
            clear_id_field: self.clear_id_field,
            press_enter: self.press_enter,
            input_method: self.input_method,
            key_delay_ms: self.key_delay_ms,
            enter_delay_ms: self.enter_delay_ms,
            tab_delay_ms: self.tab_delay_ms,
            clear_delay_ms: self.clear_delay_ms,
            key_hold_ms: self.key_hold_ms,
            auto_lock_minutes: self.auto_lock_minutes,
            require_login_screen: self.require_login_screen,
        }
    }

    pub fn from_vault_settings(&mut self, vs: &VaultSettings) {
        self.sequence = vs.sequence;
        self.clear_id_field = vs.clear_id_field;
        self.press_enter = vs.press_enter;
        self.input_method = vs.input_method;
        self.key_delay_ms = vs.key_delay_ms;
        self.enter_delay_ms = vs.enter_delay_ms;
        self.tab_delay_ms = vs.tab_delay_ms;
        self.clear_delay_ms = vs.clear_delay_ms;
        self.key_hold_ms = vs.key_hold_ms;
        self.auto_lock_minutes = vs.auto_lock_minutes;
        self.require_login_screen = vs.require_login_screen;
    }
}

#[derive(Serialize, Deserialize)]
struct PlainSettingsJson {
    #[serde(default = "default_theme_str")]
    theme: String,
    #[serde(rename = "hotkeyVk", default = "default_hotkey_vk")]
    hotkey_vk: u32,
    #[serde(rename = "hotkeyMods", default = "default_hotkey_mods")]
    hotkey_mods: u32,
    #[serde(rename = "hideFromCapture", default)]
    hide_from_capture: bool,
}

fn default_theme_str() -> String {
    "dark".to_owned()
}

fn default_hotkey_vk() -> u32 {
    'L' as u32
}

fn default_hotkey_mods() -> u32 {
    MOD_CONTROL | MOD_SHIFT
}

pub fn load_settings(path: &Path) -> Settings {
    let mut s = Settings::default();
    let Ok(text) = storage::read_text_file(path) else {
        return s;
    };
    let Ok(parsed) = serde_json::from_str::<PlainSettingsJson>(&text) else {
        return s;
    };

    s.dark_theme = parsed.theme != "light";
    s.hotkey_mods = parsed.hotkey_mods & MOD_MASK;
    s.hotkey_vk = if parsed.hotkey_vk == 0 || parsed.hotkey_vk > 0xFE {
        'L' as u32
    } else {
        parsed.hotkey_vk
    };
    s.hide_from_capture = parsed.hide_from_capture;
    s
}

pub fn save_settings(path: &Path, s: &Settings) -> std::io::Result<()> {
    let json_obj = PlainSettingsJson {
        theme: if s.dark_theme { "dark" } else { "light" }.to_owned(),
        hotkey_vk: s.hotkey_vk,
        hotkey_mods: s.hotkey_mods & MOD_MASK,
        hide_from_capture: s.hide_from_capture,
    };
    let serialized = serde_json::to_string_pretty(&json_obj)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    storage::write_text_file_atomic(path, &serialized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn settings_json_roundtrip_and_sanitization() {
        let dir = std::env::temp_dir().join("orlogin_test_settings");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("ORLogin.settings.json");

        let mut s = Settings::default();
        s.dark_theme = false;
        s.hotkey_vk = 0x74; // F5
        s.hotkey_mods = MOD_ALT | MOD_SHIFT;
        s.hide_from_capture = false;
        // Tamper-sensitive fields should NOT be persisted in plain JSON.
        s.auto_lock_minutes = 0;
        save_settings(&path, &s).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("autoLockMinutes"));
        assert!(!raw.contains("keyDelayMs"));

        let loaded = load_settings(&path);
        assert!(!loaded.dark_theme);
        assert_eq!(loaded.hotkey_vk, 0x74);
        assert_eq!(loaded.hotkey_mods, MOD_ALT | MOD_SHIFT);
        assert!(!loaded.hide_from_capture);
        assert_eq!(loaded.auto_lock_minutes, 15);

        let _ = fs::remove_dir_all(&dir);
    }
}
