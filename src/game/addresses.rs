/// Static memory addresses for OSRO clients.
/// Discovered via live memory diffing of OSRO Highrate (2018-06-21 client).

#[derive(Debug, Clone, Copy)]
pub struct ServerProfile {
    pub label: &'static str,
    pub short_label: &'static str,
    pub process_name: &'static str,
    pub in_game_address: u32,
    pub mode_address: u32,
    pub online_address: u32,
    pub login_focus_address: u32,
    pub name_address: u32,
    pub map_address: u32,
    pub char_slot_address: u32,
}

pub const HIGH_RATE: ServerProfile = ServerProfile {
    label: "Highrate",
    short_label: "HR",
    process_name: "OSRO.exe",
    in_game_address: 0x010D_CCBC,     // 1 = InGame, 0 = LoggedOut / Login Screen
    mode_address: 0x0107_AA84,        // 2 = InGame, 0 = LoggedOut / Login Screen
    online_address: 0x010A_2FB0,      // ORTools online address (>0 when online)
    login_focus_address: 0x0107_A13C, // 5 = User ID focused, 0 = Password focused
    name_address: 0x010D_F5D8,
    map_address: 0x010D_856C,
    char_slot_address: 0x010D_F5F6,
};

pub const MID_RATE: ServerProfile = ServerProfile {
    label: "Midrate",
    short_label: "MR",
    process_name: "OSRO Midrate.exe",
    in_game_address: 0x00E8_84B1,
    mode_address: 0x00E8_84B1,
    online_address: 0x00E8_84B1,
    login_focus_address: 0x0000_0000,
    name_address: 0x00E9_1C00,
    map_address: 0x00E8_ABD4,
    char_slot_address: 0x00E9_1C1E,
};
