use crate::game::addresses::ServerProfile;
use crate::game::reader::ProcessReader;
use std::collections::{BTreeSet, HashMap};
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, RECT};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetWindowThreadProcessId, IsWindow,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    NoAccess,
    AtLogin,
    InGame,
}

impl ClientState {
    pub fn label(self) -> &'static str {
        match self {
            ClientState::NoAccess => "No access",
            ClientState::AtLogin => "Login / Char select",
            ClientState::InGame => "In game",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginFieldFocus {
    Unknown,
    UserId,
    Password,
}

impl LoginFieldFocus {
    pub fn label(self) -> &'static str {
        match self {
            LoginFieldFocus::UserId => "User ID",
            LoginFieldFocus::Password => "Password",
            LoginFieldFocus::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInfo {
    pub pid: u32,
    pub hwnd_raw: isize,
    pub state: ClientState,
    pub login_focus: LoginFieldFocus,
    pub char_name: String,
    pub map: String,
    pub char_slot: u8,
}

impl ClientInfo {
    pub fn hwnd(&self) -> Option<HWND> {
        if self.hwnd_raw == 0 {
            None
        } else {
            Some(HWND(self.hwnd_raw as *mut _))
        }
    }
}

pub struct ClientMonitor {
    profile: ServerProfile,
    readers: HashMap<u32, ProcessReader>,
    clients: Vec<ClientInfo>,
}

impl ClientMonitor {
    pub fn new(profile: ServerProfile) -> Self {
        Self {
            profile,
            readers: HashMap::new(),
            clients: Vec::new(),
        }
    }

    pub fn profile(&self) -> ServerProfile {
        self.profile
    }

    pub fn clients(&self) -> &[ClientInfo] {
        &self.clients
    }

    pub fn find_by_pid(&self, pid: u32) -> Option<&ClientInfo> {
        self.clients.iter().find(|c| c.pid == pid)
    }

    pub fn refresh(&mut self) -> &[ClientInfo] {
        let alive = enumerate_matching_pids(self.profile.process_name);

        // Drop readers for processes that exited or disappeared.
        self.readers
            .retain(|pid, reader| alive.contains(pid) && !reader.has_exited());

        let mut next = Vec::with_capacity(alive.len());
        for pid in alive {
            if !self.readers.contains_key(&pid) {
                if let Some(reader) = ProcessReader::open(pid) {
                    self.readers.insert(pid, reader);
                }
            }

            let hwnd_raw = find_main_window(pid);
            let mut info = ClientInfo {
                pid,
                hwnd_raw,
                state: ClientState::NoAccess,
                login_focus: LoginFieldFocus::Unknown,
                char_name: String::new(),
                map: String::new(),
                char_slot: 0,
            };

            if let Some(reader) = self.readers.get(&pid) {
                info.state = read_client_state(reader, &self.profile, &mut info);
            }
            if info.state == ClientState::NoAccess {
                self.readers.remove(&pid); // retry opening on next refresh
            }
            next.push(info);
        }

        self.clients = next;
        &self.clients
    }
}

fn read_client_state(
    reader: &ProcessReader,
    profile: &ServerProfile,
    info: &mut ClientInfo,
) -> ClientState {
    let in_game = if profile.in_game_address != 0 {
        reader.read_u32(profile.in_game_address)
    } else {
        None
    };
    let mode = if profile.mode_address != 0 {
        reader.read_u32(profile.mode_address)
    } else {
        None
    };
    let online = if profile.online_address != 0 {
        reader.read_u32(profile.online_address)
    } else {
        None
    };

    if in_game.is_none() && mode.is_none() && online.is_none() {
        return ClientState::NoAccess;
    }

    let is_online = online.is_some_and(|v| v > 0)
        || in_game.is_some_and(|v| v == 1)
        || mode.is_some_and(|v| v == 2);

    if is_online {
        info.char_name = reader.read_cp1252_string(profile.name_address, 24);
        info.map = reader.read_cp1252_string(profile.map_address, 16);
        info.char_slot = if profile.char_slot_address != 0 {
            reader.read_u8(profile.char_slot_address).unwrap_or(0)
        } else {
            0
        };
        info.login_focus = LoginFieldFocus::Unknown;
        return ClientState::InGame;
    }

    info.login_focus = if profile.login_focus_address != 0 {
        match reader.read_u32(profile.login_focus_address) {
            Some(5) => LoginFieldFocus::UserId,
            Some(0) => LoginFieldFocus::Password,
            _ => LoginFieldFocus::Unknown,
        }
    } else {
        LoginFieldFocus::Unknown
    };

    ClientState::AtLogin
}

fn enumerate_matching_pids(process_name: &str) -> BTreeSet<u32> {
    let mut alive = BTreeSet::new();
    let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return alive;
    };
    if snap.is_invalid() {
        return alive;
    }

    let mut pe = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    unsafe {
        if Process32FirstW(snap, &mut pe).is_ok() {
            loop {
                let len = pe
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(pe.szExeFile.len());
                let exe = String::from_utf16_lossy(&pe.szExeFile[..len]);
                if exe.eq_ignore_ascii_case(process_name) {
                    alive.insert(pe.th32ProcessID);
                }
                if Process32NextW(snap, &mut pe).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }

    alive
}

struct EnumCtx {
    pid: u32,
    found: isize,
}

unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut EnumCtx);
    let mut wpid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut wpid));
    if wpid == ctx.pid && IsWindow(hwnd).as_bool() {
        let mut rc = RECT::default();
        if GetClientRect(hwnd, &mut rc).is_ok() {
            let w = rc.right - rc.left;
            let h = rc.bottom - rc.top;
            if w > 200 && h > 200 {
                ctx.found = hwnd.0 as isize;
                return BOOL(0); // Stop enumerating
            }
        }
    }
    BOOL(1)
}

pub fn find_main_window(pid: u32) -> isize {
    let mut ctx = EnumCtx { pid, found: 0 };
    unsafe {
        let _ = EnumWindows(Some(enum_window_proc), LPARAM(&mut ctx as *mut _ as isize));
    }
    ctx.found
}
