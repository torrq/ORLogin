use crate::config::{self, InputMethod, LoginSequence, Settings};
use crate::game::addresses::HIGH_RATE;
use crate::game::monitor::{ClientInfo, ClientMonitor, ClientState, LoginFieldFocus};
use crate::input::typer::{Typer, TyperJob};
use crate::security::secure::{wipe_string, ProtectedSecret, SecureBytes};
use crate::security::vault::{Account, Vault};
use crate::storage;
use crate::theme::{
    self, primary_button, render_lock_icon_button, render_theme_icon_button, svg_action_button,
    SvgIconCache, ThemePalette, DELETE_ICON_SVG, EDIT_ICON_SVG, SEND_ICON_SVG,
};
use crate::win_bridge::{self, HotkeyCapture, WinBridge, WinEvent};
use crossbeam_channel::Receiver;
use eframe::egui::{self, Color32, RichText, Rounding, Stroke, Vec2};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

const VERSION_LABEL: &str = "v6";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortColumn {
    Label,
    Username,
}

struct StatusMessage {
    text: String,
    color: Color32,
    expiry: Instant,
}

pub struct ORLoginApp {
    hwnd_raw: isize,
    settings_path: PathBuf,
    settings: Settings,
    vault: Vault,
    monitor: ClientMonitor,
    typer: Typer,
    bridge: WinBridge,
    win_rx: Receiver<WinEvent>,

    // Unlock / Create / Change Master form buffers (wiped on lock / submit / exit)
    pw_buf1: String,
    pw_buf2: String,
    pw_buf3: String,
    wipe_epoch: u64,
    focus_master_input: bool,
    failed_unlocks: u32,
    cooldown_until: Option<Instant>,

    // Add / Edit Account modal state
    account_form_open: bool,
    focus_account_field: bool,
    editing_index: Option<usize>,
    label_buf: String,
    user_buf: String,
    acc_pw_buf: String,
    show_acc_pw: bool,

    // Inline delete confirmation
    confirm_delete_index: Option<usize>,

    // Table sorting
    sort_col: SortColumn,
    sort_asc: bool,

    // Modals
    change_master_open: bool,
    advanced_settings_open: bool,

    // Hotkey capture state
    capturing_hotkey: bool,
    hotkey_registered: bool,

    // Cached rasterized SVG UI icons
    icons: SvgIconCache,

    // Timing
    last_client_refresh: Instant,
    last_activity: Instant,

    status: Option<StatusMessage>,
}

impl Drop for ORLoginApp {
    fn drop(&mut self) {
        self.wipe_form_buffers();
        self.vault.lock();
    }
}

impl ORLoginApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        storage_dir: PathBuf,
        settings: Settings,
    ) -> Self {
        theme::configure_fonts(&cc.egui_ctx);
        theme::apply_theme(&cc.egui_ctx, settings.dark_theme);

        let hwnd_raw = cc
            .window_handle()
            .ok()
            .and_then(|wh| match wh.as_raw() {
                RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
                _ => None,
            })
            .unwrap_or(0);

        let (bridge, win_rx) = WinBridge::spawn(
            cc.egui_ctx.clone(),
            settings.hotkey_mods,
            settings.hotkey_vk,
        );

        let mut monitor = ClientMonitor::new(HIGH_RATE);
        monitor.refresh();

        let app = Self {
            hwnd_raw,
            settings_path: storage_dir.join("ORLogin.settings.json"),
            settings,
            vault: Vault::new(storage_dir.join("ORLogin.vault")),
            monitor,
            typer: Typer::new(),
            bridge,
            win_rx,
            pw_buf1: String::new(),
            pw_buf2: String::new(),
            pw_buf3: String::new(),
            wipe_epoch: 0,
            focus_master_input: true,
            failed_unlocks: 0,
            cooldown_until: None,
            account_form_open: false,
            focus_account_field: false,
            editing_index: None,
            label_buf: String::new(),
            user_buf: String::new(),
            acc_pw_buf: String::new(),
            show_acc_pw: false,
            confirm_delete_index: None,
            sort_col: SortColumn::Label,
            sort_asc: true,
            change_master_open: false,
            advanced_settings_open: false,
            capturing_hotkey: false,
            hotkey_registered: false,
            icons: SvgIconCache::default(),
            last_client_refresh: Instant::now(),
            last_activity: Instant::now(),
            status: None,
        };

        app.apply_settings_to_system();
        app
    }

    fn palette(&self) -> ThemePalette {
        ThemePalette::get(self.settings.dark_theme)
    }

    fn flash_status(&mut self, text: impl Into<String>, color: Color32) {
        self.status = Some(StatusMessage {
            text: text.into(),
            color,
            expiry: Instant::now() + Duration::from_secs(4),
        });
    }

    fn apply_settings_to_system(&self) {
        if self.hwnd_raw != 0 {
            win_bridge::apply_titlebar_dark_mode(self.hwnd_raw, self.settings.dark_theme);
            win_bridge::set_window_display_affinity(
                self.hwnd_raw,
                self.settings.hide_from_capture,
            );
        }
    }

    fn save_settings(&mut self) {
        let _ = config::save_settings(&self.settings_path, &self.settings);
        if self.vault.is_unlocked() {
            self.vault.set_settings(self.settings.to_vault_settings());
            let _ = self.vault.save();
        }
    }

    fn touch_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    fn wipe_form_buffers(&mut self) {
        wipe_string(&mut self.pw_buf1);
        wipe_string(&mut self.pw_buf2);
        wipe_string(&mut self.pw_buf3);
        wipe_string(&mut self.acc_pw_buf);
        self.label_buf.zeroize();
        self.label_buf.clear();
        self.user_buf.zeroize();
        self.user_buf.clear();
        self.show_acc_pw = false;
        self.wipe_epoch = self.wipe_epoch.wrapping_add(1);
    }

    fn lock_vault(&mut self, reason: Option<&str>) {
        self.account_form_open = false;
        self.editing_index = None;
        self.confirm_delete_index = None;
        self.change_master_open = false;
        self.advanced_settings_open = false;
        if self.capturing_hotkey {
            self.capturing_hotkey = false;
            self.bridge
                .set_hotkey(self.settings.hotkey_mods, self.settings.hotkey_vk);
        }
        self.focus_master_input = true;
        self.wipe_form_buffers();
        self.vault.lock();
        if let Some(msg) = reason {
            let color = self.palette().amber;
            self.flash_status(msg, color);
        }
    }

    fn check_auto_lock(&mut self) {
        if !self.vault.is_unlocked() || self.settings.auto_lock_minutes <= 0 {
            return;
        }
        let limit = Duration::from_secs(self.settings.auto_lock_minutes as u64 * 60);
        if self.last_activity.elapsed() >= limit {
            self.lock_vault(Some("Vault auto-locked due to inactivity"));
        }
    }

    fn send_to_login_client(&mut self, account_idx: usize) {
        self.touch_activity();
        let pal = self.palette();

        if !self.vault.is_unlocked() || account_idx >= self.vault.accounts().len() {
            return;
        }
        if self.typer.is_busy() {
            self.flash_status("Already typing, please wait...", pal.amber);
            return;
        }

        self.monitor.refresh();
        self.last_client_refresh = Instant::now();

        let target = self
            .monitor
            .clients()
            .iter()
            .find(|cl| cl.state == ClientState::AtLogin && cl.hwnd_raw != 0)
            .cloned();

        let Some(target) = target else {
            self.flash_status("No client at login screen ready for login", pal.amber);
            return;
        };

        self.dispatch_login_to_client(account_idx, &target, true);
    }

    fn dispatch_login_to_client(
        &mut self,
        account_idx: usize,
        target: &ClientInfo,
        activate_window: bool,
    ) {
        let pal = self.palette();

        if target.state == ClientState::InGame {
            let who = if target.char_name.is_empty() {
                String::new()
            } else {
                format!(" ({})", target.char_name)
            };
            self.flash_status(format!("Blocked: client is already in-game{who}"), pal.amber);
            return;
        }

        if target.state != ClientState::AtLogin || target.hwnd_raw == 0 {
            self.flash_status("Blocked: client is not at login screen", pal.amber);
            return;
        }

        if (activate_window || self.settings.input_method == InputMethod::SendInput)
            && win_bridge::get_foreground_hwnd_raw() != target.hwnd_raw
        {
            win_bridge::set_foreground_window(target.hwnd_raw);
        }

        let acc = &self.vault.accounts()[account_idx];
        let Ok(pw) = acc.password.reveal() else {
            self.flash_status("Failed to decrypt account password in memory", pal.red);
            return;
        };
        let who = if acc.label.is_empty() {
            acc.username.clone()
        } else {
            format!("{} ({})", acc.label, acc.username)
        };

        let job = TyperJob {
            hwnd_raw: target.hwnd_raw,
            target_pid: target.pid,
            expected_exe: self.monitor.profile().process_name,
            username: acc.username.clone(),
            password: pw,
            sequence: self.settings.sequence,
            current_focus: target.login_focus,
            clear_id_field: self.settings.clear_id_field,
            press_enter: self.settings.press_enter,
            method: self.settings.input_method,
            key_delay_ms: self.settings.key_delay_ms,
            enter_delay_ms: self.settings.enter_delay_ms,
            tab_delay_ms: self.settings.tab_delay_ms,
            clear_delay_ms: self.settings.clear_delay_ms,
            key_hold_ms: self.settings.key_hold_ms,
        };

        if self.typer.start(job) {
            self.flash_status(
                format!("Sent login for {who} -> PID {}", target.pid),
                pal.green,
            );
        }
    }

    fn on_hotkey(&mut self) {
        self.touch_activity();
        let pal = self.palette();

        if !self.vault.is_unlocked() {
            self.flash_status("Hotkey ignored: vault is locked", pal.amber);
            return;
        }
        if self.vault.accounts().is_empty() {
            self.flash_status("Hotkey ignored: no accounts saved", pal.amber);
            return;
        }

        let mut active_idx = self.vault.active_index();
        if active_idx < 0 || active_idx as usize >= self.vault.accounts().len() {
            active_idx = 0;
        }

        if self.typer.is_busy() {
            self.flash_status("Already typing, please wait...", pal.amber);
            return;
        }

        self.monitor.refresh();
        self.last_client_refresh = Instant::now();

        let fg_pid = win_bridge::get_foreground_pid();
        let mut target = self.monitor.find_by_pid(fg_pid).cloned();

        // Convenience fallback: if unfocused using PostMessage and exactly 1 client is at Login
        if target.is_none() && self.settings.input_method == InputMethod::PostMessage {
            let login_clients: Vec<_> = self
                .monitor
                .clients()
                .iter()
                .filter(|cl| cl.state == ClientState::AtLogin && cl.hwnd_raw != 0)
                .collect();
            if login_clients.len() == 1 {
                target = Some(login_clients[0].clone());
            }
        }

        let Some(target) = target.filter(|t| t.hwnd_raw != 0) else {
            self.flash_status("Focus a Highrate client at the login screen first", pal.amber);
            return;
        };

        self.dispatch_login_to_client(active_idx as usize, &target, false);
    }

    fn open_add_account_form(&mut self) {
        self.wipe_form_buffers();
        self.account_form_open = true;
        self.editing_index = None;
        self.confirm_delete_index = None;
        self.focus_account_field = true;
    }

    fn open_edit_account_form(&mut self, index: usize) {
        let Some(acc) = self.vault.accounts().get(index) else {
            return;
        };
        let label = acc.label.clone();
        let username = acc.username.clone();
        let Ok(pw) = acc.password.reveal() else {
            let red = self.palette().red;
            self.flash_status("Failed to decrypt password in memory", red);
            return;
        };

        self.wipe_form_buffers();
        self.account_form_open = true;
        self.editing_index = Some(index);
        self.confirm_delete_index = None;
        self.focus_account_field = true;

        self.label_buf = label;
        self.user_buf = username;
        if let Ok(pw_str) = std::str::from_utf8(pw.as_slice()) {
            self.acc_pw_buf.push_str(pw_str);
        }
    }

    fn render_header(&mut self, ui: &mut egui::Ui, pal: ThemePalette) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("OSRO Login Helper")
                    .color(pal.accent)
                    .size(16.0)
                    .strong(),
            );
            ui.label(RichText::new(VERSION_LABEL).color(pal.muted));

            // Server badge
            ui.add(
                egui::Button::new(RichText::new(self.monitor.profile().label).color(pal.accent))
                    .fill(pal.panel)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .sense(egui::Sense::hover()),
            );

            ui.add_space(8.0);
            ui.label(RichText::new("Hotkey:").color(pal.muted));

            let hk_label = if self.capturing_hotkey {
                "Press keys... (Esc)".to_owned()
            } else {
                win_bridge::format_hotkey(self.settings.hotkey_mods, self.settings.hotkey_vk)
            };
            if ui
                .button(&hk_label)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Click to rebind login hotkey")
                .clicked()
            {
                self.capturing_hotkey = true;
                self.bridge.set_hotkey(0, 0);
            }
            if !self.hotkey_registered && !self.capturing_hotkey {
                ui.label(RichText::new("(conflict)").color(pal.red));
            }

            if self.capturing_hotkey {
                match win_bridge::poll_hotkey_capture() {
                    HotkeyCapture::Waiting => {}
                    HotkeyCapture::Cancelled => {
                        self.capturing_hotkey = false;
                        self.bridge
                            .set_hotkey(self.settings.hotkey_mods, self.settings.hotkey_vk);
                    }
                    HotkeyCapture::Captured { mods, vk } => {
                        self.settings.hotkey_mods = mods;
                        self.settings.hotkey_vk = vk;
                        self.capturing_hotkey = false;
                        self.bridge.set_hotkey(mods, vk);
                        self.save_settings();
                    }
                }
            }

            // Right-aligned controls
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if render_theme_icon_button(ui, self.settings.dark_theme, pal) {
                    self.settings.dark_theme = !self.settings.dark_theme;
                    self.apply_settings_to_system();
                    self.save_settings();
                }

                if self.vault.is_unlocked() {
                    if render_lock_icon_button(ui, pal) {
                        self.lock_vault(Some("Vault locked"));
                    }

                    if ui
                        .button("Master Password")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.wipe_form_buffers();
                        self.change_master_open = true;
                    }

                    if ui
                        .button("Advanced...")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(
                            "Configure keystroke injection method, sequence, safety checks & delays",
                        )
                        .clicked()
                    {
                        self.advanced_settings_open = true;
                    }
                }
            });
        });
    }

    fn render_create_vault_screen(&mut self, ui: &mut egui::Ui, pal: ThemePalette) {
        let card_w = 500.0f32.min(ui.available_width() - 16.0);
        let top_pad = ((ui.available_height() - 360.0) * 0.42).max(16.0);

        ui.vertical_centered(|ui| {
            ui.add_space(top_pad);
            egui::Frame::group(ui.style())
                .fill(pal.surface)
                .stroke(Stroke::new(1.0_f32, pal.border))
                .rounding(Rounding::same(6.0))
                .inner_margin(egui::Margin::same(16.0))
                .show(ui, |ui| {
                    ui.set_width(card_w);
                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.label(
                            RichText::new("Create Encrypted Vault")
                                .color(pal.accent)
                                .size(15.0)
                                .strong(),
                        );
                        ui.label("Accounts and passwords are encrypted with AES-256-GCM using a key derived from your master password (PBKDF2-HMAC-SHA256, 600,000 iterations).");
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Important: There is no password recovery. If you forget your master password, the vault cannot be decrypted.",
                            )
                            .color(pal.amber),
                        );
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(6.0);

                        ui.label("Master Password");
                        let pw1_resp = ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::TextEdit::singleline(&mut self.pw_buf1)
                                .password(true)
                                .id(egui::Id::new(("create_pw1", self.wipe_epoch))),
                        );
                        if self.focus_master_input {
                            pw1_resp.request_focus();
                            self.focus_master_input = false;
                        }

                        let (strength, strength_label) = estimate_strength(&self.pw_buf1);
                        ui.add_space(4.0);
                        ui.add(
                            egui::ProgressBar::new(strength)
                                .desired_width(ui.available_width())
                                .desired_height(6.0),
                        );
                        ui.label(RichText::new(format!("Strength: {strength_label}")).color(pal.muted).small());

                        ui.add_space(6.0);
                        ui.label("Confirm Master Password");
                        let pw2_resp = ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::TextEdit::singleline(&mut self.pw_buf2)
                                .password(true)
                                .id(egui::Id::new(("create_pw2", self.wipe_epoch))),
                        );
                        let submit_enter =
                            pw2_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                        ui.add_space(10.0);
                        let clicked = primary_button(
                            ui,
                            "Create Vault",
                            pal,
                            Some(Vec2::new(ui.available_width(), 34.0)),
                        )
                        .clicked();

                        if clicked || submit_enter {
                            if self.pw_buf1.len() < 8 {
                                self.flash_status(
                                    "Master password must be at least 8 characters",
                                    pal.red,
                                );
                            } else if self.pw_buf1 != self.pw_buf2 {
                                self.flash_status("Passwords do not match", pal.red);
                            } else {
                                let Ok(p1) = SecureBytes::from_slice(self.pw_buf1.as_bytes()) else {
                                    self.flash_status("Memory allocation failure", pal.red);
                                    return;
                                };
                                self.wipe_form_buffers();
                                match self.vault.create(p1.as_slice(), self.settings.to_vault_settings()) {
                                    Ok(()) => {
                                        self.touch_activity();
                                        self.flash_status("Encrypted vault created", pal.green);
                                    }
                                    Err(e) => self.flash_status(e.as_str(), pal.red),
                                }
                            }
                        }
                    });
                });
        });
    }

    fn render_unlock_screen(&mut self, ui: &mut egui::Ui, pal: ThemePalette) {
        let card_w = 460.0f32.min(ui.available_width() - 16.0);
        let top_pad = ((ui.available_height() - 240.0) * 0.42).max(16.0);

        ui.vertical_centered(|ui| {
            ui.add_space(top_pad);
            egui::Frame::group(ui.style())
                .fill(pal.surface)
                .stroke(Stroke::new(1.0_f32, pal.border))
                .rounding(Rounding::same(6.0))
                .inner_margin(egui::Margin::same(16.0))
                .show(ui, |ui| {
                    ui.set_width(card_w);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Vault Locked")
                                .color(pal.accent)
                                .size(15.0)
                                .strong(),
                        );
                        ui.label(
                            RichText::new("Enter your master password to decrypt saved accounts.")
                                .color(pal.muted),
                        );
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(6.0);

                        let now = Instant::now();
                        let cooling_down = self.cooldown_until.is_some_and(|t| now < t);

                        ui.label("Master Password");
                        let pw_resp = ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::TextEdit::singleline(&mut self.pw_buf1)
                                .password(true)
                                .horizontal_align(egui::Align::Center)
                                .id(egui::Id::new(("unlock_pw", self.wipe_epoch))),
                        );
                        if self.focus_master_input && !cooling_down {
                            pw_resp.request_focus();
                            self.focus_master_input = false;
                        }
                        let submit_enter = !cooling_down
                            && pw_resp.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter));

                        ui.add_space(10.0);
                        if let Some(until) = self.cooldown_until.filter(|&t| now < t) {
                            let secs = (until - now).as_secs_f64();
                            ui.add_enabled_ui(false, |ui| {
                                primary_button(
                                    ui,
                                    &format!("Please wait {secs:.1}s..."),
                                    pal,
                                    Some(Vec2::new(ui.available_width(), 34.0)),
                                );
                            });
                        } else {
                            let clicked = primary_button(
                                ui,
                                "Unlock Vault",
                                pal,
                                Some(Vec2::new(ui.available_width(), 34.0)),
                            )
                            .clicked();

                            if clicked || submit_enter {
                                let Ok(master) = SecureBytes::from_slice(self.pw_buf1.as_bytes())
                                else {
                                    self.flash_status("Memory allocation failure", pal.red);
                                    return;
                                };
                                self.wipe_form_buffers();

                                match self.vault.unlock(master.as_slice()) {
                                    Ok(()) => {
                                        self.failed_unlocks = 0;
                                        self.cooldown_until = None;
                                        self.touch_activity();
                                        if self.vault.has_loaded_settings() {
                                            let vs = self.vault.settings().clone();
                                            self.settings.from_vault_settings(&vs);
                                            self.apply_settings_to_system();
                                        }
                                        self.flash_status("Vault unlocked", pal.green);
                                    }
                                    Err(e) => {
                                        self.failed_unlocks += 1;
                                        if self.failed_unlocks >= 3 {
                                            let exp = (self.failed_unlocks - 2).min(5);
                                            let delay_sec = (1u64 << exp).min(30);
                                            self.cooldown_until =
                                                Some(Instant::now() + Duration::from_secs(delay_sec));
                                        }
                                        self.focus_master_input = true;
                                        self.flash_status(e.as_str(), pal.red);
                                    }
                                }
                            }
                        }
                    });
                });
        });
    }

    fn render_accounts_panel(&mut self, ui: &mut egui::Ui, pal: ThemePalette) {
        self.vault.normalize_active();
        let count = self.vault.accounts().len();

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Saved Accounts ({count})"))
                    .color(pal.accent)
                    .strong(),
            );
            ui.label(
                RichText::new(format!(
                    "- active account fires on {}",
                    win_bridge::format_hotkey(self.settings.hotkey_mods, self.settings.hotkey_vk)
                ))
                .color(pal.muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary_button(ui, "+ Add Account", pal, None).clicked() {
                    self.open_add_account_form();
                }
            });
        });
        ui.separator();

        if count == 0 {
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "No accounts saved yet. Click '+ Add Account' above to add your first login.",
                )
                .color(pal.muted),
            );
            return;
        }

        // Compute display order sorted case-insensitively
        let mut display_order: Vec<usize> = (0..count).collect();
        let accounts = self.vault.accounts();
        let sort_col = self.sort_col;
        let sort_asc = self.sort_asc;
        display_order.sort_by(|&a_idx, &b_idx| {
            let a = &accounts[a_idx];
            let b = &accounts[b_idx];
            let ord = match sort_col {
                SortColumn::Label => cmp_nocase(&a.label, &b.label)
                    .then_with(|| cmp_nocase(&a.username, &b.username)),
                SortColumn::Username => cmp_nocase(&a.username, &b.username)
                    .then_with(|| cmp_nocase(&a.label, &b.label)),
            };
            if sort_asc {
                ord
            } else {
                ord.reverse()
            }
        });

        let mut new_active: Option<i32> = None;
        let mut trigger_send: Option<i32> = None;
        let mut trigger_edit: Option<usize> = None;
        let mut trigger_delete_confirm: Option<usize> = None;

        let active_idx = self.vault.active_index();
        let edit_blue = if pal.dark { pal.accent } else { pal.primary };
        let has_login_client = self
            .monitor
            .clients()
            .iter()
            .any(|cl| cl.state == ClientState::AtLogin && cl.hwnd_raw != 0);
        let can_send = has_login_client && !self.typer.is_busy();

        let active_w = 54.0_f32;
        let actions_w = 224.0_f32;
        let gap = 10.0_f32;

        let compute_cols = |rect: egui::Rect| -> [egui::Rect; 4] {
            let x0 = rect.min.x + 8.0;
            let x_end = (rect.max.x - 8.0).max(x0 + active_w + actions_w + 80.0);
            let y0 = rect.min.y;
            let y1 = rect.max.y;

            let c_active = egui::Rect::from_min_max(egui::Pos2::new(x0, y0), egui::Pos2::new(x0 + active_w, y1));
            let c_actions =
                egui::Rect::from_min_max(egui::Pos2::new(x_end - actions_w, y0), egui::Pos2::new(x_end, y1));

            let flex_start = c_active.max.x + gap;
            let flex_end = (c_actions.min.x - gap).max(flex_start + 60.0);
            let flex_total = flex_end - flex_start;
            let label_w = (flex_total * 0.42).floor();

            let c_label =
                egui::Rect::from_min_max(egui::Pos2::new(flex_start, y0), egui::Pos2::new(flex_start + label_w, y1));
            let c_user = egui::Rect::from_min_max(
                egui::Pos2::new(c_label.max.x + gap, y0),
                egui::Pos2::new(flex_end, y1),
            );
            [c_active, c_label, c_user, c_actions]
        };

        let table_w = ui.available_width().max(340.0);
        let div_col = Color32::from_rgba_unmultiplied(pal.border.r(), pal.border.g(), pal.border.b(), 80);

        // Pinned full-width column header bar
        let (hdr_rect, _) = ui.allocate_exact_size(Vec2::new(table_w, 28.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(hdr_rect, Rounding::same(4.0), pal.panel);
        let hdr_cols = compute_cols(hdr_rect);
        for c in &hdr_cols[..3] {
            let x = c.max.x + gap * 0.5;
            ui.painter().line_segment(
                [egui::Pos2::new(x, hdr_rect.min.y + 5.0), egui::Pos2::new(x, hdr_rect.max.y - 5.0)],
                Stroke::new(1.0_f32, div_col),
            );
        }

        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(hdr_cols[0])
                .layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)),
            |ui| {
                ui.label(RichText::new("Active").strong());
            },
        );

        for (col_rect, col, title) in [
            (hdr_cols[1], SortColumn::Label, "Label"),
            (hdr_cols[2], SortColumn::Username, "Username"),
        ] {
            let hit_rect = egui::Rect::from_min_max(
                egui::Pos2::new(col_rect.min.x - gap * 0.4, hdr_rect.min.y),
                egui::Pos2::new(col_rect.max.x + gap * 0.4, hdr_rect.max.y),
            );
            let resp = ui.interact(
                hit_rect,
                egui::Id::new(("acct_sort_hdr", title)),
                egui::Sense::click(),
            );
            if resp.hovered() {
                ui.painter()
                    .rect_filled(hit_rect, Rounding::same(3.0), pal.panel_hover);
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let active = self.sort_col == col;
            let arrow = if active {
                if self.sort_asc { "  \u{25B2}" } else { "  \u{25BC}" }
            } else {
                ""
            };
            let text_col = if active || resp.hovered() {
                pal.accent
            } else {
                pal.text
            };
            ui.allocate_new_ui(
                egui::UiBuilder::new()
                    .max_rect(col_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                |ui| {
                    ui.label(
                        RichText::new(format!("{title}{arrow}"))
                            .color(text_col)
                            .strong(),
                    );
                },
            );
            if resp.clicked() {
                if self.sort_col == col {
                    self.sort_asc = !self.sort_asc;
                } else {
                    self.sort_col = col;
                    self.sort_asc = true;
                }
            }
        }

        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(hdr_cols[3])
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.label(RichText::new("Actions").strong());
            },
        );

        ui.add_space(2.0);

        let alt_row_bg = Color32::from_rgba_unmultiplied(pal.panel.r(), pal.panel.g(), pal.panel.b(), 85);
        let hover_row_bg =
            Color32::from_rgba_unmultiplied(pal.panel_hover.r(), pal.panel_hover.g(), pal.panel_hover.b(), 140);
        let line_col = Color32::from_rgba_unmultiplied(pal.border.r(), pal.border.g(), pal.border.b(), 110);

        egui::ScrollArea::vertical()
            .id_salt("accounts_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (row_idx, &i) in display_order.iter().enumerate() {
                    let acc = &accounts[i];
                    let row_h = 34.0_f32;
                    let row_w = ui.available_width().max(328.0);
                    let (row_rect, row_resp) =
                        ui.allocate_exact_size(Vec2::new(row_w, row_h), egui::Sense::hover());
                    let hovered = row_resp.hovered() || ui.rect_contains_pointer(row_rect);

                    let bg = if hovered {
                        hover_row_bg
                    } else if row_idx % 2 == 1 {
                        alt_row_bg
                    } else {
                        Color32::TRANSPARENT
                    };
                    if bg != Color32::TRANSPARENT {
                        ui.painter().rect_filled(row_rect, Rounding::same(4.0), bg);
                    }
                    ui.painter().line_segment(
                        [
                            egui::Pos2::new(row_rect.min.x, row_rect.max.y),
                            egui::Pos2::new(row_rect.max.x, row_rect.max.y),
                        ],
                        Stroke::new(1.0_f32, line_col),
                    );

                    let cols = compute_cols(row_rect);

                    let select_rect = egui::Rect::from_min_max(
                        row_rect.min,
                        egui::Pos2::new(cols[3].min.x - gap * 0.5, row_rect.max.y),
                    );
                    let row_click = ui.interact(
                        select_rect,
                        egui::Id::new(("acct_row_sel", i)),
                        egui::Sense::click(),
                    );
                    if row_click.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if row_click.clicked() {
                        new_active = Some(i as i32);
                    }

                    ui.allocate_new_ui(
                        egui::UiBuilder::new()
                            .max_rect(cols[0])
                            .layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)),
                        |ui| {
                            if ui
                                .radio(i as i32 == active_idx, "")
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                new_active = Some(i as i32);
                            }
                        },
                    );

                    ui.allocate_new_ui(
                        egui::UiBuilder::new()
                            .max_rect(cols[1])
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        |ui| {
                            ui.set_clip_rect(cols[1].intersect(ui.clip_rect()));
                            if acc.label.is_empty() {
                                ui.add(
                                    egui::Label::new(RichText::new("-").color(pal.muted))
                                        .selectable(false),
                                );
                            } else {
                                ui.add(
                                    egui::Label::new(&acc.label)
                                        .truncate()
                                        .selectable(false),
                                );
                            }
                        },
                    );

                    ui.allocate_new_ui(
                        egui::UiBuilder::new()
                            .max_rect(cols[2])
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        |ui| {
                            ui.set_clip_rect(cols[2].intersect(ui.clip_rect()));
                            ui.add(
                                egui::Label::new(&acc.username)
                                    .truncate()
                                    .selectable(false),
                            );
                        },
                    );

                    ui.allocate_new_ui(
                        egui::UiBuilder::new()
                            .max_rect(cols[3])
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            if self.confirm_delete_index == Some(i) {
                                ui.label(RichText::new("Delete?").color(pal.red));
                                let yes_btn = egui::Button::new(RichText::new("Yes").color(pal.red))
                                    .stroke(Stroke::new(1.0_f32, pal.red))
                                    .rounding(Rounding::ZERO);
                                if ui
                                    .add_sized([46.0, 26.0], yes_btn)
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    trigger_delete_confirm = Some(i);
                                }
                                let no_btn = egui::Button::new("No").rounding(Rounding::ZERO);
                                if ui
                                    .add_sized([46.0, 26.0], no_btn)
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.confirm_delete_index = None;
                                }
                            } else {
                                let send_resp = svg_action_button(
                                    ui,
                                    &mut self.icons,
                                    "send",
                                    SEND_ICON_SVG,
                                    "Send",
                                    pal.green,
                                    pal,
                                    Vec2::new(68.0, 26.0),
                                    can_send,
                                );
                                if !has_login_client {
                                    send_resp.on_hover_text("No client at login screen");
                                } else if send_resp.clicked() {
                                    trigger_send = Some(i as i32);
                                }

                                if svg_action_button(
                                    ui,
                                    &mut self.icons,
                                    "edit",
                                    EDIT_ICON_SVG,
                                    "Edit",
                                    edit_blue,
                                    pal,
                                    Vec2::new(62.0, 26.0),
                                    true,
                                )
                                .clicked()
                                {
                                    trigger_edit = Some(i);
                                }
                                if svg_action_button(
                                    ui,
                                    &mut self.icons,
                                    "delete",
                                    DELETE_ICON_SVG,
                                    "Delete",
                                    pal.red,
                                    pal,
                                    Vec2::new(76.0, 26.0),
                                    true,
                                )
                                .clicked()
                                {
                                    self.confirm_delete_index = Some(i);
                                }
                            }
                        },
                    );
                }
            });

        if let Some(idx) = new_active {
            self.vault.set_active_index(idx);
            let _ = self.vault.save();
        }
        if let Some(idx) = trigger_send {
            self.vault.set_active_index(idx);
            let _ = self.vault.save();
            self.send_to_login_client(idx as usize);
        }
        if let Some(idx) = trigger_edit {
            self.open_edit_account_form(idx);
        }
        if let Some(idx) = trigger_delete_confirm {
            if idx < self.vault.accounts().len() {
                self.vault.accounts_mut().remove(idx); // Account Drop zeroizes label, user, password
                let cur_active = self.vault.active_index();
                if cur_active == idx as i32 {
                    self.vault.set_active_index(0);
                } else if cur_active > idx as i32 {
                    self.vault.set_active_index(cur_active - 1);
                }
                self.vault.normalize_active();
                self.confirm_delete_index = None;
                let _ = self.vault.save();
                self.flash_status("Account deleted", pal.amber);
            }
        }
    }

    fn render_clients_panel(&mut self, ui: &mut egui::Ui, pal: ThemePalette) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Running Clients ({})", self.monitor.clients().len()))
                    .color(pal.accent)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Refresh")
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.monitor.refresh();
                    self.last_client_refresh = Instant::now();
                }
            });
        });
        ui.separator();

        let clients = self.monitor.clients();
        if clients.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("No OSRO client processes detected.").color(pal.muted));
            return;
        }

        let fg_hwnd = win_bridge::get_foreground_hwnd_raw();
        egui::ScrollArea::vertical()
            .id_salt("clients_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for cl in clients {
                    let focused = cl.hwnd_raw != 0 && cl.hwnd_raw == fg_hwnd;
                    let (dot_color, status) = match cl.state {
                        ClientState::NoAccess => (pal.red, cl.state.label().to_owned()),
                        ClientState::AtLogin => {
                            let s = match cl.login_focus {
                                LoginFieldFocus::UserId => "At Login (User ID)",
                                LoginFieldFocus::Password => "At Login (Password)",
                                LoginFieldFocus::Unknown => "At Login",
                            };
                            (pal.amber, s.to_owned())
                        }
                        ClientState::InGame => {
                            let s = if !cl.char_name.is_empty() {
                                if !cl.map.is_empty() {
                                    format!("Logged in: {} @ {}", cl.char_name, cl.map)
                                } else {
                                    format!("Logged in: {}", cl.char_name)
                                }
                            } else {
                                "Logged in".to_owned()
                            };
                            (pal.green, s)
                        }
                    };

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(RichText::new("\u{25CF}").color(dot_color));
                        ui.label(RichText::new(format!("PID {}", cl.pid)).strong());
                        if focused {
                            egui::Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(
                                    pal.accent.r(),
                                    pal.accent.g(),
                                    pal.accent.b(),
                                    32,
                                ))
                                .stroke(Stroke::new(1.0_f32, pal.accent))
                                .rounding(Rounding::same(3.0))
                                .inner_margin(egui::Margin::symmetric(5.0, 1.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Focused").color(pal.accent).small());
                                });
                        }
                        let st_resp = ui.add(
                            egui::Label::new(RichText::new(&status).color(dot_color)).truncate(),
                        );
                        if cl.state == ClientState::InGame {
                            st_resp.on_hover_text(format!(
                                "Character: {}\nSlot: {}\nMap: {}",
                                cl.char_name, cl.char_slot, cl.map
                            ));
                        }
                    });
                }
            });
    }

    fn render_settings_panel(&mut self, ui: &mut egui::Ui, pal: ThemePalette) {
        ui.label(RichText::new("Settings").color(pal.accent).strong());
        ui.separator();
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            let mut auto_lock_enabled = self.settings.auto_lock_minutes > 0;
            if ui
                .checkbox(&mut auto_lock_enabled, "Auto-lock vault")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .changed()
            {
                self.settings.auto_lock_minutes = if auto_lock_enabled { 15 } else { 0 };
                self.save_settings();
            }
            if auto_lock_enabled {
                ui.spacing_mut().slider_width = 96.0;
                let slider_changed = ui
                    .add(egui::Slider::new(&mut self.settings.auto_lock_minutes, 1..=60).show_value(false))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .changed();
                let val_changed = ui
                    .add_sized(
                        [62.0, 22.0],
                        egui::DragValue::new(&mut self.settings.auto_lock_minutes)
                            .range(1..=60)
                            .suffix(" min"),
                    )
                    .changed();
                if slider_changed || val_changed {
                    self.save_settings();
                }
            }
        });

        ui.add_space(2.0);
        if ui
            .checkbox(&mut self.settings.hide_from_capture, "Hide from screen capture")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .changed()
        {
            self.apply_settings_to_system();
            self.save_settings();
        }
    }

    fn render_modals(&mut self, ctx: &egui::Context, pal: ThemePalette) {
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        let mut backdrop_clicked = false;
        if self.account_form_open || self.change_master_open || self.advanced_settings_open {
            egui::Area::new(egui::Id::new("modal_backdrop"))
                .fixed_pos(egui::Pos2::ZERO)
                .order(egui::Order::Middle)
                .show(ctx, |ui| {
                    let screen = ui.ctx().screen_rect();
                    let resp = ui.allocate_rect(screen, egui::Sense::click());
                    backdrop_clicked = resp.clicked();
                    ui.painter().rect_filled(
                        screen,
                        0.0,
                        Color32::from_rgba_unmultiplied(pal.base.r(), pal.base.g(), pal.base.b(), 190),
                    );
                });
        }

        if backdrop_clicked || esc {
            if self.account_form_open || self.change_master_open {
                self.wipe_form_buffers();
            }
            self.account_form_open = false;
            self.change_master_open = false;
            self.advanced_settings_open = false;
            return;
        }

        // 1. Account Details Modal
        if self.account_form_open {
            {
                let is_new = self.editing_index.is_none();
                let mut open = self.account_form_open;
                let win_resp = egui::Window::new("Account Details")
                    .open(&mut open)
                    .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                    .fixed_size(Vec2::new(420.0, 0.0))
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(if is_new { "Add New Account" } else { "Edit Account" })
                                    .color(pal.accent)
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(if is_new {
                                    "Credentials are AES-256-GCM encrypted in your vault."
                                } else {
                                    "Update your stored credentials."
                                })
                                .color(pal.muted),
                            );
                        });
                        ui.separator();

                        ui.label("Label (optional)");
                        let lbl_r = ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::TextEdit::singleline(&mut self.label_buf)
                                .hint_text("e.g. Main / Priest / Vendor"),
                        );
                        if self.focus_account_field {
                            lbl_r.request_focus();
                            self.focus_account_field = false;
                        }

                        ui.add_space(4.0);
                        ui.label("Username / ID");
                        let usr_r = ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::TextEdit::singleline(&mut self.user_buf)
                                .hint_text("Game account username"),
                        );

                        ui.add_space(4.0);
                        ui.label("Password");
                        let pw_r = ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::TextEdit::singleline(&mut self.acc_pw_buf)
                                .password(!self.show_acc_pw)
                                .hint_text("Game account password")
                                .id(egui::Id::new(("acc_pw", self.wipe_epoch))),
                        );

                        ui.checkbox(&mut self.show_acc_pw, "Show password")
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        ui.separator();

                        let enter_pressed = (lbl_r.lost_focus() || usr_r.lost_focus() || pw_r.lost_focus())
                            && ui.input(|i| i.key_pressed(egui::Key::Enter));

                        ui.horizontal(|ui| {
                            let primary_w = 112.0_f32;
                            let cancel_w = 78.0_f32;
                            let btn_gap = 8.0_f32;
                            let pad = ((ui.available_width() - (primary_w + btn_gap + cancel_w)) * 0.5).max(0.0);
                            ui.add_space(pad);
                            ui.spacing_mut().item_spacing.x = btn_gap;

                            let submit_btn = primary_button(
                                ui,
                                if is_new { "Add Account" } else { "Save Changes" },
                                pal,
                                Some(Vec2::new(primary_w, 28.0)),
                            )
                            .clicked();

                            if submit_btn || enter_pressed {
                                if self.user_buf.is_empty() {
                                    self.flash_status("Username cannot be empty", pal.red);
                                } else if self.acc_pw_buf.is_empty() {
                                    self.flash_status("Password cannot be empty", pal.red);
                                } else {
                                    let pw_sec = ProtectedSecret::from_slice(self.acc_pw_buf.as_bytes());
                                    match pw_sec {
                                        Err(_) => self.flash_status(
                                            "Failed to securely protect password in memory",
                                            pal.red,
                                        ),
                                        Ok(protected_pw) => {
                                            let acc = Account {
                                                label: self.label_buf.clone(),
                                                username: self.user_buf.clone(),
                                                password: protected_pw,
                                            };
                                            match self.editing_index {
                                                None => {
                                                    self.vault.accounts_mut().push(acc);
                                                    if self.vault.active_index() < 0 {
                                                        let idx = self.vault.accounts().len() as i32 - 1;
                                                        self.vault.set_active_index(idx);
                                                    }
                                                }
                                                Some(idx) if idx < self.vault.accounts().len() => {
                                                    self.vault.accounts_mut()[idx] = acc;
                                                }
                                                _ => {}
                                            }
                                            self.vault.normalize_active();
                                            self.account_form_open = false;
                                            self.wipe_form_buffers();
                                            match self.vault.save() {
                                                Ok(()) => self.flash_status(
                                                    "Account saved & encrypted",
                                                    pal.green,
                                                ),
                                                Err(e) => self.flash_status(e.as_str(), pal.red),
                                            }
                                        }
                                    }
                                }
                            }

                            if ui
                                .add_sized([78.0, 28.0], egui::Button::new("Cancel"))
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.account_form_open = false;
                                self.wipe_form_buffers();
                            }
                        });
                    });
                apply_window_close_hand_cursor(ctx, win_resp.as_ref().map(|ir| &ir.response));
                if !open && self.account_form_open {
                    self.account_form_open = false;
                    self.wipe_form_buffers();
                }
            }
        }

        // 2. Change Master Password Modal
        if self.change_master_open {
            if esc {
                self.change_master_open = false;
                self.wipe_form_buffers();
            } else {
                let mut open = self.change_master_open;
                let win_resp = egui::Window::new("Change Master Password")
                    .open(&mut open)
                    .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                    .fixed_size(Vec2::new(420.0, 0.0))
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        ui.label("Current Master Password");
                        ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::TextEdit::singleline(&mut self.pw_buf1)
                                .password(true)
                                .id(egui::Id::new(("cur_mp", self.wipe_epoch))),
                        );

                        ui.add_space(4.0);
                        ui.label("New Master Password");
                        ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::TextEdit::singleline(&mut self.pw_buf2)
                                .password(true)
                                .id(egui::Id::new(("new_mp1", self.wipe_epoch))),
                        );

                        ui.add_space(4.0);
                        ui.label("Confirm New Master Password");
                        let mp2 = ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::TextEdit::singleline(&mut self.pw_buf3)
                                .password(true)
                                .id(egui::Id::new(("new_mp2", self.wipe_epoch))),
                        );
                        let enter_pressed =
                            mp2.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                        ui.separator();
                        ui.horizontal(|ui| {
                            let submit = primary_button(ui, "Update Master Password", pal, None).clicked();
                            if submit || enter_pressed {
                                if self.pw_buf2.len() < 8 {
                                    self.flash_status(
                                        "New master password must be at least 8 characters",
                                        pal.red,
                                    );
                                } else if self.pw_buf2 != self.pw_buf3 {
                                    self.flash_status("New passwords do not match", pal.red);
                                } else {
                                    let cur = SecureBytes::from_slice(self.pw_buf1.as_bytes());
                                    let next = SecureBytes::from_slice(self.pw_buf2.as_bytes());
                                    self.wipe_form_buffers();
                                    match (cur, next) {
                                        (Ok(c_sec), Ok(n_sec)) => {
                                            match self
                                                .vault
                                                .change_master_password(c_sec.as_slice(), n_sec.as_slice())
                                            {
                                                Ok(()) => {
                                                    self.change_master_open = false;
                                                    self.flash_status(
                                                        "Master password updated & vault re-encrypted",
                                                        pal.green,
                                                    );
                                                }
                                                Err(e) => self.flash_status(e.as_str(), pal.red),
                                            }
                                        }
                                        _ => self.flash_status("Memory allocation failure", pal.red),
                                    }
                                }
                            }
                            if ui
                                .button("Cancel")
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.change_master_open = false;
                                self.wipe_form_buffers();
                            }
                        });
                    });
                apply_window_close_hand_cursor(ctx, win_resp.as_ref().map(|ir| &ir.response));
                if !open && self.change_master_open {
                    self.change_master_open = false;
                    self.wipe_form_buffers();
                }
            }
        }

        // 3. Advanced Settings Modal
        if self.advanced_settings_open {
            if esc {
                self.advanced_settings_open = false;
            } else {
                let mut open = self.advanced_settings_open;
                let mut changed = false;
                let mut close_and_save = false;
                let mut reset_defaults = false;

                let win_resp = egui::Window::new("Advanced Settings")
                    .open(&mut open)
                    .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                    .fixed_size(Vec2::new(510.0, 0.0))
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        let card_bg = Color32::from_rgba_unmultiplied(
                            pal.panel.r(),
                            pal.panel.g(),
                            pal.panel.b(),
                            110,
                        );
                        let card_frame = egui::Frame::none()
                            .fill(card_bg)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .rounding(Rounding::same(5.0))
                            .inner_margin(egui::Margin::same(10.0));

                        // Section 1: Input & Sequence
                        ui.label(
                            RichText::new("Input & Credential Sequence")
                                .color(pal.accent)
                                .strong(),
                        );
                        ui.add_space(2.0);
                        card_frame.show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            egui::Grid::new("adv_input_grid")
                                .num_columns(2)
                                .spacing([14.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label("Input Method");
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            changed |= ui
                                                .radio_value(
                                                    &mut self.settings.input_method,
                                                    InputMethod::PostMessage,
                                                    "PostMessage",
                                                )
                                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                .changed();
                                            ui.add_space(8.0);
                                            changed |= ui
                                                .radio_value(
                                                    &mut self.settings.input_method,
                                                    InputMethod::SendInput,
                                                    "SendInput",
                                                )
                                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                .changed();
                                        });
                                        let hint = match self.settings.input_method {
                                            InputMethod::PostMessage => {
                                                "Sends keystrokes directly to the client window (works unfocused)."
                                            }
                                            InputMethod::SendInput => {
                                                "Synthesizes hardware scancodes while the client stays focused."
                                            }
                                        };
                                        ui.label(RichText::new(hint).color(pal.muted).small());
                                    });
                                    ui.end_row();

                                    ui.label("Sequence");
                                    ui.horizontal(|ui| {
                                        changed |= ui
                                            .radio_value(
                                                &mut self.settings.sequence,
                                                LoginSequence::IdTabPassword,
                                                "ID + Password",
                                            )
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .changed();
                                        ui.add_space(8.0);
                                        changed |= ui
                                            .radio_value(
                                                &mut self.settings.sequence,
                                                LoginSequence::PasswordOnly,
                                                "Password only",
                                            )
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .changed();
                                    });
                                    ui.end_row();

                                    ui.label("Options");
                                    ui.vertical(|ui| {
                                        if self.settings.sequence == LoginSequence::IdTabPassword {
                                            changed |= ui
                                                .checkbox(
                                                    &mut self.settings.clear_id_field,
                                                    "Clear ID field first",
                                                )
                                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                .changed();
                                        }
                                        changed |= ui
                                            .checkbox(
                                                &mut self.settings.press_enter,
                                                "Press Enter twice (confirm Service Select)",
                                            )
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .changed();
                                    });
                                    ui.end_row();
                                });
                        });

                        ui.add_space(6.0);
                        egui::Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(
                                pal.green.r(),
                                pal.green.g(),
                                pal.green.b(),
                                24,
                            ))
                            .stroke(Stroke::new(
                                1.0_f32,
                                Color32::from_rgba_unmultiplied(
                                    pal.green.r(),
                                    pal.green.g(),
                                    pal.green.b(),
                                    110,
                                ),
                            ))
                            .rounding(Rounding::same(4.0))
                            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(
                                    RichText::new(
                                        "Login Screen Protection: Enforced (in-game typing strictly blocked)",
                                    )
                                    .color(pal.green)
                                    .small(),
                                );
                            });

                        ui.add_space(8.0);

                        // Section 2: Timing & Delays
                        ui.label(
                            RichText::new("Timing & Delays")
                                .color(pal.accent)
                                .strong(),
                        );
                        ui.add_space(2.0);
                        card_frame.show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let slider_w = (ui.available_width() - 168.0 - 88.0).clamp(130.0, 230.0);
                            egui::Grid::new("adv_timing_grid")
                                .num_columns(2)
                                .min_col_width(148.0)
                                .spacing([12.0, 7.0])
                                .show(ui, |ui| {
                                    changed |= timing_slider_row(
                                        ui,
                                        "Character Typing",
                                        "Delay between typed characters (default: 15 ms)",
                                        &mut self.settings.key_delay_ms,
                                        1..=60,
                                        slider_w,
                                    );
                                    changed |= timing_slider_row(
                                        ui,
                                        "Double-Enter Pause",
                                        "Pause between 1st Return (credentials) and 2nd Return (service select). Default: 300 ms",
                                        &mut self.settings.enter_delay_ms,
                                        50..=1000,
                                        slider_w,
                                    );
                                    changed |= timing_slider_row(
                                        ui,
                                        "Tab Switch Delay",
                                        "Pause after Tab field-switch and focus alignment (default: 25 ms)",
                                        &mut self.settings.tab_delay_ms,
                                        5..=200,
                                        slider_w,
                                    );
                                    changed |= timing_slider_row(
                                        ui,
                                        "Clear Backspace",
                                        "Delay between backspaces when clearing the ID field (default: 3 ms)",
                                        &mut self.settings.clear_delay_ms,
                                        1..=30,
                                        slider_w,
                                    );
                                    changed |= timing_slider_row(
                                        ui,
                                        "Hardware Key Hold",
                                        "Key-down hold duration before key-up release (default: 10 ms)",
                                        &mut self.settings.key_hold_ms,
                                        2..=50,
                                        slider_w,
                                    );
                                });
                        });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button("Reset to Defaults")
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                reset_defaults = true;
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if primary_button(ui, "Save & Close", pal, None).clicked() {
                                    close_and_save = true;
                                }
                            });
                        });
                    });
                apply_window_close_hand_cursor(ctx, win_resp.as_ref().map(|ir| &ir.response));
                if !open {
                    self.advanced_settings_open = false;
                }
                if reset_defaults {
                    self.settings.key_delay_ms = 15;
                    self.settings.enter_delay_ms = 300;
                    self.settings.tab_delay_ms = 25;
                    self.settings.clear_delay_ms = 3;
                    self.settings.key_hold_ms = 10;
                    self.settings.input_method = InputMethod::PostMessage;
                    self.settings.sequence = LoginSequence::IdTabPassword;
                    self.settings.clear_id_field = true;
                    self.settings.press_enter = true;
                    self.settings.require_login_screen = true;
                    self.save_settings();
                    self.flash_status("Advanced settings reset to defaults", pal.green);
                } else if close_and_save {
                    self.save_settings();
                    self.advanced_settings_open = false;
                    self.flash_status("Advanced settings saved", pal.green);
                } else if changed {
                    self.save_settings();
                }
            }
        }
    }
}

impl eframe::App for ORLoginApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Track user input for inactivity auto-lock
        let had_input = ctx.input(|i| {
            i.pointer.any_click()
                || i.raw_scroll_delta != Vec2::ZERO
                || !i.keys_down.is_empty()
        });
        if had_input {
            self.touch_activity();
        }

        // Process Win32 bridge events (WM_HOTKEY, WM_WTSSESSION_CHANGE, registration status)
        while let Ok(ev) = self.win_rx.try_recv() {
            match ev {
                WinEvent::HotkeyPressed => self.on_hotkey(),
                WinEvent::SessionLocked => {
                    self.lock_vault(Some("Vault locked (Windows session locked)"));
                }
                WinEvent::HotkeyStatus(ok) => self.hotkey_registered = ok,
            }
        }

        // Periodic client memory poll (every 500 ms)
        if self.last_client_refresh.elapsed() >= Duration::from_millis(500) {
            self.monitor.refresh();
            self.last_client_refresh = Instant::now();
        }
        self.check_auto_lock();

        // Keep periodic repaints running for client monitoring, auto-lock, and cooldown timers
        if self.capturing_hotkey {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        let pal = theme::apply_theme(ctx, self.settings.dark_theme);

        // Top Header Bar
        egui::TopBottomPanel::top("header_bar")
            .frame(
                egui::Frame::none()
                    .fill(pal.base)
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0)),
            )
            .show(ctx, |ui| {
                self.render_header(ui, pal);
                ui.add_space(4.0);
                ui.separator();
            });

        // Bottom Centered Status Bar
        egui::TopBottomPanel::bottom("status_bar")
            .frame(
                egui::Frame::none()
                    .fill(pal.base)
                    .inner_margin(egui::Margin::symmetric(14.0, 6.0)),
            )
            .show(ctx, |ui| {
                let (text, color) = match &self.status {
                    Some(st) if Instant::now() < st.expiry => (st.text.clone(), st.color),
                    _ => {
                        let mode = if storage::is_portable_mode() {
                            " [Portable]"
                        } else {
                            ""
                        };
                        let label = if self.vault.is_unlocked() {
                            format!("Vault: Unlocked (AES-256-GCM){mode}")
                        } else {
                            format!("Vault: Locked{mode}")
                        };
                        (label, pal.muted)
                    }
                };
                ui.vertical_centered(|ui| {
                    ui.colored_label(color, text);
                });
            });

        // Bottom Split Cards (Running Clients + Settings) when Unlocked
        if self.vault.is_unlocked() {
            egui::TopBottomPanel::bottom("bottom_cards")
                .frame(
                    egui::Frame::none()
                        .fill(pal.base)
                        .inner_margin(egui::Margin::symmetric(14.0, 4.0)),
                )
                .show(ctx, |ui| {
                    let bottom_h = 120.0_f32;
                    let total_w = ui.available_width();
                    let gap = 10.0_f32;
                    let clients_w = (total_w * 0.60)
                        .clamp(340.0, (total_w - 250.0).max(340.0))
                        .min(total_w - 210.0);
                    let settings_w = (total_w - clients_w - gap).max(180.0);

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;

                        ui.allocate_ui_with_layout(
                            Vec2::new(clients_w, bottom_h),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                egui::Frame::group(ui.style())
                                    .fill(pal.surface)
                                    .stroke(Stroke::new(1.0_f32, pal.border))
                                    .rounding(Rounding::same(6.0))
                                    .inner_margin(egui::Margin::same(10.0))
                                    .show(ui, |ui| {
                                        let inner = ui.available_size();
                                        ui.set_min_size(inner);
                                        ui.set_max_size(inner);
                                        self.render_clients_panel(ui, pal);
                                    });
                            },
                        );

                        ui.allocate_ui_with_layout(
                            Vec2::new(settings_w, bottom_h),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                egui::Frame::group(ui.style())
                                    .fill(pal.surface)
                                    .stroke(Stroke::new(1.0_f32, pal.border))
                                    .rounding(Rounding::same(6.0))
                                    .inner_margin(egui::Margin::same(10.0))
                                    .show(ui, |ui| {
                                        let inner = ui.available_size();
                                        ui.set_min_size(inner);
                                        ui.set_max_size(inner);
                                        self.render_settings_panel(ui, pal);
                                    });
                            },
                        );
                    });
                });
        }

        // Central Panel
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(pal.base)
                    .inner_margin(egui::Margin::symmetric(14.0, 6.0)),
            )
            .show(ctx, |ui| {
                if !self.vault.exists() {
                    self.render_create_vault_screen(ui, pal);
                } else if !self.vault.is_unlocked() {
                    self.render_unlock_screen(ui, pal);
                } else {
                    egui::Frame::group(ui.style())
                        .fill(pal.surface)
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .rounding(Rounding::same(6.0))
                        .inner_margin(egui::Margin::same(12.0))
                        .show(ui, |ui| {
                            let inner = ui.available_size();
                            ui.set_min_size(inner);
                            ui.set_max_size(inner);
                            self.render_accounts_panel(ui, pal);
                        });
                }
            });

        if self.vault.is_unlocked() {
            self.render_modals(ctx, pal);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.wipe_form_buffers();
        self.vault.lock();
    }
}

fn apply_window_close_hand_cursor(ctx: &egui::Context, resp: Option<&egui::Response>) {
    let Some(r) = resp else {
        return;
    };
    // Account for `window_margin` (14 px) around TitleBar so the hit rect covers the entire 'x' button.
    let close_rect = egui::Rect::from_min_max(
        egui::Pos2::new(r.rect.max.x - 40.0, r.rect.min.y + 12.0),
        egui::Pos2::new(r.rect.max.x - 12.0, r.rect.min.y + 40.0),
    );
    if ctx.rect_contains_pointer(r.layer_id, close_rect) {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

fn timing_slider_row(
    ui: &mut egui::Ui,
    label: &str,
    tooltip: &str,
    val: &mut i32,
    range: std::ops::RangeInclusive<i32>,
    slider_w: f32,
) -> bool {
    let mut changed = false;
    ui.label(label).on_hover_text(tooltip);
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = slider_w;
        changed |= ui
            .add(egui::Slider::new(val, range.clone()).show_value(false))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .changed();
        changed |= ui
            .add_sized(
                [66.0, 22.0],
                egui::DragValue::new(val).range(range).suffix(" ms"),
            )
            .changed();
    });
    ui.end_row();
    changed
}

fn estimate_strength(pw: &str) -> (f32, &'static str) {
    let mut lower = false;
    let mut upper = false;
    let mut digit = false;
    let mut symbol = false;
    for b in pw.bytes() {
        if b.is_ascii_lowercase() {
            lower = true;
        } else if b.is_ascii_uppercase() {
            upper = true;
        } else if b.is_ascii_digit() {
            digit = true;
        } else {
            symbol = true;
        }
    }
    let classes = lower as u8 + upper as u8 + digit as u8 + symbol as u8;
    let mut score = (pw.len() as f32 / 16.0).min(1.0) * 0.65 + (classes as f32 / 4.0) * 0.35;
    if pw.len() < 8 {
        score = score.min(0.25);
        (score, "Weak (under 8 chars)")
    } else if score < 0.55 {
        (score, "Fair")
    } else if score < 0.80 {
        (score, "Good")
    } else {
        (score, "Strong")
    }
}

fn cmp_nocase(a: &str, b: &str) -> std::cmp::Ordering {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let mut ia = 0usize;
    let mut ib = 0usize;

    while ia < ab.len() && ib < bb.len() {
        let ca = ab[ia];
        let cb = bb[ib];

        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let start_a = ia;
            while ia < ab.len() && ab[ia].is_ascii_digit() {
                ia += 1;
            }
            let start_b = ib;
            while ib < bb.len() && bb[ib].is_ascii_digit() {
                ib += 1;
            }

            let mut trim_a = start_a;
            while trim_a < ia && ab[trim_a] == b'0' {
                trim_a += 1;
            }
            let mut trim_b = start_b;
            while trim_b < ib && bb[trim_b] == b'0' {
                trim_b += 1;
            }

            let sig_ord = (ia - trim_a)
                .cmp(&(ib - trim_b))
                .then_with(|| ab[trim_a..ia].cmp(&bb[trim_b..ib]));
            if sig_ord != std::cmp::Ordering::Equal {
                return sig_ord;
            }
            let raw_len_ord = (ia - start_a).cmp(&(ib - start_b));
            if raw_len_ord != std::cmp::Ordering::Equal {
                return raw_len_ord.reverse();
            }
        } else {
            let ord = ca.to_ascii_lowercase().cmp(&cb.to_ascii_lowercase());
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
            ia += 1;
            ib += 1;
        }
    }
    ab.len().cmp(&bb.len())
}

#[cfg(test)]
mod tests {
    use super::cmp_nocase;

    #[test]
    fn natural_sorting_orders_numbers_correctly() {
        let mut items = ["t10", "t2", "T1", "t9", "t11", "t3", "acc20", "acc3"];
        items.sort_by(|a, b| cmp_nocase(a, b));
        assert_eq!(items, ["acc3", "acc20", "T1", "t2", "t3", "t9", "t10", "t11"]);
    }
}
