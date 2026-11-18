use crate::config::{InputMethod, LoginSequence, VaultSettings};
use crate::security::crypto::{
    aes_gcm_decrypt, aes_gcm_encrypt, pbkdf2_sha256, random_bytes, CryptoError, K_KEY_SIZE,
    K_NONCE_SIZE, K_SALT_SIZE, K_TAG_SIZE,
};
use crate::security::secure::{wipe_string, ProtectedSecret, SecureBytes};
use crate::storage;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use zeroize::Zeroize;

const MAGIC: [u8; 4] = *b"ORLV";
const FORMAT_VERSION: u16 = 1;
const KDF_PBKDF2: u16 = 1;
const HEADER_SIZE: usize = 44;
const PAYLOAD_VERSION_1: u32 = 1;
const PAYLOAD_VERSION_2: u32 = 2;
const MAX_CIPHERTEXT: usize = 16 * 1024 * 1024;
const MAX_ACCOUNTS: usize = 10_000;
const MAX_FIELD_LEN: usize = 4096;

pub const DEFAULT_ITERATIONS: u32 = 600_000;
pub const MIN_ITERATIONS: u32 = 100_000;
pub const MAX_ITERATIONS: u32 = 50_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultError {
    NotFound,
    WrongPassword,
    Corrupt,
    IoError,
    CryptoError,
}

impl VaultError {
    pub fn as_str(self) -> &'static str {
        match self {
            VaultError::NotFound => "Vault file not found",
            VaultError::WrongPassword => "Wrong master password (or the vault was modified)",
            VaultError::Corrupt => "Vault file is corrupt or unsupported",
            VaultError::IoError => "Could not read/write the vault file",
            VaultError::CryptoError => "Windows cryptography error",
        }
    }
}

pub struct Account {
    pub label: String,
    pub username: String,
    pub password: ProtectedSecret,
}

impl Drop for Account {
    fn drop(&mut self) {
        wipe_string(&mut self.label);
        wipe_string(&mut self.username);
        self.password.clear();
    }
}

pub struct Vault {
    path: PathBuf,
    unlocked: bool,
    key: ProtectedSecret,
    salt: [u8; K_SALT_SIZE],
    iterations: u32,
    accounts: Vec<Account>,
    active: i32,
    settings: VaultSettings,
    has_settings: bool,
}

impl Drop for Vault {
    fn drop(&mut self) {
        self.lock();
    }
}

impl Vault {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            unlocked: false,
            key: ProtectedSecret::new(),
            salt: [0u8; K_SALT_SIZE],
            iterations: DEFAULT_ITERATIONS,
            accounts: Vec::new(),
            active: -1,
            settings: VaultSettings::default(),
            has_settings: false,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    pub fn is_unlocked(&self) -> bool {
        self.unlocked
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    pub fn accounts_mut(&mut self) -> &mut Vec<Account> {
        &mut self.accounts
    }

    pub fn active_index(&self) -> i32 {
        self.active
    }

    pub fn set_active_index(&mut self, idx: i32) {
        self.active = idx;
        self.normalize_active();
    }

    pub fn normalize_active(&mut self) {
        let n = self.accounts.len() as i32;
        if n == 0 {
            self.active = -1;
        } else if n == 1 {
            self.active = 0;
        } else if self.active >= n {
            self.active = n - 1;
        } else if self.active < -1 {
            self.active = -1;
        }
    }

    pub fn settings(&self) -> &VaultSettings {
        &self.settings
    }

    pub fn set_settings(&mut self, s: VaultSettings) {
        self.settings = s;
    }

    pub fn has_loaded_settings(&self) -> bool {
        self.has_settings
    }

    pub fn create(&mut self, master: &[u8], initial_settings: VaultSettings) -> Result<(), VaultError> {
        self.lock();

        let mut salt = [0u8; K_SALT_SIZE];
        random_bytes(&mut salt).map_err(|_| VaultError::CryptoError)?;

        let key = derive_key(master, &salt, DEFAULT_ITERATIONS)?;
        self.salt = salt;
        self.iterations = DEFAULT_ITERATIONS;
        if self.key.set(key.as_slice()).is_err() {
            self.lock();
            return Err(VaultError::CryptoError);
        }
        self.accounts.clear();
        self.active = -1;
        self.settings = initial_settings;
        self.has_settings = true;
        self.unlocked = true;

        if let Err(e) = self.save() {
            self.lock();
            return Err(e);
        }
        Ok(())
    }

    pub fn unlock(&mut self, master: &[u8]) -> Result<(), VaultError> {
        self.lock();

        let file = read_vault_file(&self.path)?;
        if file.len() < HEADER_SIZE + K_TAG_SIZE {
            return Err(VaultError::Corrupt);
        }
        if file[0..4] != MAGIC {
            return Err(VaultError::Corrupt);
        }
        if get_u16(&file[4..6]) != FORMAT_VERSION || get_u16(&file[6..8]) != KDF_PBKDF2 {
            return Err(VaultError::Corrupt);
        }

        let iterations = get_u32(&file[8..12]);
        if !(MIN_ITERATIONS..=MAX_ITERATIONS).contains(&iterations) {
            return Err(VaultError::Corrupt);
        }
        let mut salt = [0u8; K_SALT_SIZE];
        salt.copy_from_slice(&file[12..28]);

        let key = derive_key(master, &salt, iterations)?;
        let plain = decrypt_with_key(&file, &key)?;

        self.deserialize_payload(plain.as_slice())?;

        self.salt = salt;
        self.iterations = iterations;
        if self.key.set(key.as_slice()).is_err() {
            self.lock();
            return Err(VaultError::CryptoError);
        }
        self.unlocked = true;
        Ok(())
    }

    pub fn save(&self) -> Result<(), VaultError> {
        if !self.unlocked {
            return Err(VaultError::CryptoError);
        }
        let plain = self.serialize_payload()?;
        let key_buf = self.key.reveal().map_err(|_| VaultError::CryptoError)?;
        if key_buf.len() != K_KEY_SIZE {
            return Err(VaultError::CryptoError);
        }
        let mut key_arr = [0u8; K_KEY_SIZE];
        key_arr.copy_from_slice(key_buf.as_slice());

        let mut nonce = [0u8; K_NONCE_SIZE];
        if random_bytes(&mut nonce).is_err() {
            key_arr.zeroize();
            return Err(VaultError::CryptoError);
        }

        let mut out = vec![0u8; HEADER_SIZE + plain.len() + K_TAG_SIZE];
        out[0..4].copy_from_slice(&MAGIC);
        put_u16(&mut out[4..6], FORMAT_VERSION);
        put_u16(&mut out[6..8], KDF_PBKDF2);
        put_u32(&mut out[8..12], self.iterations);
        out[12..28].copy_from_slice(&self.salt);
        out[28..40].copy_from_slice(&nonce);
        put_u32(&mut out[40..44], plain.len() as u32);

        let (aad_hdr, rest) = out.split_at_mut(HEADER_SIZE);
        let (ct_slice, tag_slice) = rest.split_at_mut(plain.len());
        let mut tag = [0u8; K_TAG_SIZE];
        let enc_res = aes_gcm_encrypt(
            &key_arr,
            &nonce,
            aad_hdr,
            plain.as_slice(),
            ct_slice,
            &mut tag,
        );
        key_arr.zeroize();
        enc_res.map_err(|_| VaultError::CryptoError)?;
        tag_slice.copy_from_slice(&tag);

        storage::write_bytes_atomic(&self.path, &out, true).map_err(|_| VaultError::IoError)
    }

    pub fn change_master_password(&mut self, current: &[u8], next: &[u8]) -> Result<(), VaultError> {
        if !self.unlocked {
            return Err(VaultError::CryptoError);
        }

        let check = derive_key(current, &self.salt, self.iterations)?;
        let old_key = self.key.reveal().map_err(|_| VaultError::CryptoError)?;
        if !old_key.ct_eq(&check) {
            return Err(VaultError::WrongPassword);
        }

        let mut new_salt = [0u8; K_SALT_SIZE];
        random_bytes(&mut new_salt).map_err(|_| VaultError::CryptoError)?;
        let new_key = derive_key(next, &new_salt, DEFAULT_ITERATIONS)?;

        let old_salt = self.salt;
        let old_iters = self.iterations;

        self.salt = new_salt;
        self.iterations = DEFAULT_ITERATIONS;
        if self.key.set(new_key.as_slice()).is_err() {
            self.salt = old_salt;
            self.iterations = old_iters;
            let _ = self.key.set(old_key.as_slice());
            return Err(VaultError::CryptoError);
        }

        if let Err(e) = self.save() {
            self.salt = old_salt;
            self.iterations = old_iters;
            let _ = self.key.set(old_key.as_slice());
            return Err(e);
        }

        // Refresh .bak with the newly encrypted vault so no old-password ciphertext lingers.
        let mut bak = self.path.as_os_str().to_os_string();
        bak.push(".bak");
        let _ = fs::copy(&self.path, PathBuf::from(bak));

        // Clean up any leftover .tmp file.
        let mut tmp = self.path.as_os_str().to_os_string();
        tmp.push(".tmp");
        let _ = fs::remove_file(PathBuf::from(tmp));

        Ok(())
    }

    pub fn lock(&mut self) {
        self.accounts.clear();
        self.accounts.shrink_to_fit();
        self.key.clear();
        self.salt.zeroize();
        self.active = -1;
        self.settings = VaultSettings::default();
        self.has_settings = false;
        self.unlocked = false;
    }

    fn serialize_payload(&self) -> Result<SecureBytes, VaultError> {
        let mut estimate = 12 + 44;
        for a in &self.accounts {
            estimate += 12 + a.label.len() + a.username.len() + a.password.size();
        }

        let mut b = SecureBytes::with_capacity(estimate).map_err(|_| VaultError::CryptoError)?;
        append_u32(&mut b, PAYLOAD_VERSION_2)?;
        append_u32(&mut b, self.active as u32)?;
        append_u32(&mut b, self.accounts.len() as u32)?;
        for a in &self.accounts {
            append_field(&mut b, a.label.as_bytes())?;
            append_field(&mut b, a.username.as_bytes())?;
            let pw = a.password.reveal().map_err(|_| VaultError::CryptoError)?;
            append_field(&mut b, pw.as_slice())?;
        }

        // Authenticated security settings block (payload v2)
        append_u32(&mut b, self.settings.sequence as u32)?;
        append_u32(&mut b, u32::from(self.settings.clear_id_field))?;
        append_u32(&mut b, u32::from(self.settings.press_enter))?;
        append_u32(&mut b, self.settings.input_method as u32)?;
        append_u32(&mut b, self.settings.key_delay_ms as u32)?;
        append_u32(&mut b, self.settings.enter_delay_ms as u32)?;
        append_u32(&mut b, self.settings.tab_delay_ms as u32)?;
        append_u32(&mut b, self.settings.clear_delay_ms as u32)?;
        append_u32(&mut b, self.settings.key_hold_ms as u32)?;
        append_u32(&mut b, self.settings.auto_lock_minutes as u32)?;
        append_u32(&mut b, u32::from(self.settings.require_login_screen))?;

        Ok(b)
    }

    fn deserialize_payload(&mut self, plain: &[u8]) -> Result<(), VaultError> {
        let mut r = Reader { b: plain, pos: 0 };
        let version = r.read_u32().ok_or(VaultError::Corrupt)?;
        if version != PAYLOAD_VERSION_1 && version != PAYLOAD_VERSION_2 {
            return Err(VaultError::Corrupt);
        }
        let active = r.read_u32().ok_or(VaultError::Corrupt)? as i32;
        let count = r.read_u32().ok_or(VaultError::Corrupt)? as usize;
        if count > MAX_ACCOUNTS {
            return Err(VaultError::Corrupt);
        }

        let mut accounts = Vec::with_capacity(count);
        for _ in 0..count {
            let label_bytes = r.read_field().ok_or(VaultError::Corrupt)?;
            let user_bytes = r.read_field().ok_or(VaultError::Corrupt)?;
            let pw_bytes = r.read_field().ok_or(VaultError::Corrupt)?;

            let label = String::from_utf8(label_bytes.to_vec()).map_err(|_| VaultError::Corrupt)?;
            let username =
                String::from_utf8(user_bytes.to_vec()).map_err(|_| VaultError::Corrupt)?;
            let password =
                ProtectedSecret::from_slice(pw_bytes).map_err(|_| VaultError::CryptoError)?;

            accounts.push(Account {
                label,
                username,
                password,
            });
        }

        if version == PAYLOAD_VERSION_1 {
            if r.pos != plain.len() {
                return Err(VaultError::Corrupt);
            }
            self.has_settings = false;
        } else {
            let seq = r.read_u32().ok_or(VaultError::Corrupt)?;
            let clr = r.read_u32().ok_or(VaultError::Corrupt)?;
            let enter = r.read_u32().ok_or(VaultError::Corrupt)?;
            let meth = r.read_u32().ok_or(VaultError::Corrupt)?;
            let k_del = r.read_u32().ok_or(VaultError::Corrupt)? as i32;
            let e_del = r.read_u32().ok_or(VaultError::Corrupt)? as i32;
            let t_del = r.read_u32().ok_or(VaultError::Corrupt)? as i32;
            let c_del = r.read_u32().ok_or(VaultError::Corrupt)? as i32;
            let k_hld = r.read_u32().ok_or(VaultError::Corrupt)? as i32;
            let lock_m = r.read_u32().ok_or(VaultError::Corrupt)? as i32;
            let req_login = r.read_u32().ok_or(VaultError::Corrupt)?;
            if r.pos != plain.len() {
                return Err(VaultError::Corrupt);
            }

            let mut loaded = VaultSettings {
                sequence: if seq == 1 {
                    LoginSequence::PasswordOnly
                } else {
                    LoginSequence::IdTabPassword
                },
                clear_id_field: clr != 0,
                press_enter: enter != 0,
                input_method: if meth == 1 {
                    InputMethod::SendInput
                } else {
                    InputMethod::PostMessage
                },
                key_delay_ms: k_del,
                enter_delay_ms: e_del,
                tab_delay_ms: t_del,
                clear_delay_ms: c_del,
                key_hold_ms: k_hld,
                auto_lock_minutes: lock_m,
                require_login_screen: req_login != 0,
            };
            loaded.clamp();
            self.settings = loaded;
            self.has_settings = true;
        }

        self.accounts = accounts;
        self.active = active;
        self.normalize_active();
        Ok(())
    }
}

fn derive_key(master: &[u8], salt: &[u8], iterations: u32) -> Result<SecureBytes, VaultError> {
    let mut key = SecureBytes::zeroed(K_KEY_SIZE).map_err(|_| VaultError::CryptoError)?;
    if pbkdf2_sha256(master, salt, iterations, key.as_mut_slice()).is_err() {
        return Err(VaultError::CryptoError);
    }
    Ok(key)
}

fn read_vault_file(path: &Path) -> Result<Vec<u8>, VaultError> {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(VaultError::NotFound),
        Err(_) => return Err(VaultError::IoError),
    };
    let max_file = (HEADER_SIZE + MAX_CIPHERTEXT + K_TAG_SIZE) as u64;
    if meta.len() > max_file {
        return Err(VaultError::Corrupt);
    }
    fs::read(path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            VaultError::NotFound
        } else {
            VaultError::IoError
        }
    })
}

fn decrypt_with_key(file: &[u8], key: &SecureBytes) -> Result<SecureBytes, VaultError> {
    if file.len() < HEADER_SIZE + K_TAG_SIZE || key.len() != K_KEY_SIZE {
        return Err(VaultError::Corrupt);
    }
    let ct_len = get_u32(&file[40..44]) as usize;
    if ct_len > MAX_CIPHERTEXT || file.len() != HEADER_SIZE + ct_len + K_TAG_SIZE {
        return Err(VaultError::Corrupt);
    }

    let mut key_arr = [0u8; K_KEY_SIZE];
    key_arr.copy_from_slice(key.as_slice());
    let mut nonce = [0u8; K_NONCE_SIZE];
    nonce.copy_from_slice(&file[28..40]);
    let mut tag = [0u8; K_TAG_SIZE];
    tag.copy_from_slice(&file[HEADER_SIZE + ct_len..HEADER_SIZE + ct_len + K_TAG_SIZE]);

    let mut plain = SecureBytes::zeroed(ct_len).map_err(|_| VaultError::CryptoError)?;
    let dec = aes_gcm_decrypt(
        &key_arr,
        &nonce,
        &file[..HEADER_SIZE],
        &file[HEADER_SIZE..HEADER_SIZE + ct_len],
        &tag,
        plain.as_mut_slice(),
    );
    key_arr.zeroize();

    match dec {
        Ok(()) => Ok(plain),
        Err(CryptoError::AuthFailed) => Err(VaultError::WrongPassword),
        Err(CryptoError::SystemError) => Err(VaultError::CryptoError),
    }
}

fn put_u16(dst: &mut [u8], v: u16) {
    dst[0..2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(dst: &mut [u8], v: u32) {
    dst[0..4].copy_from_slice(&v.to_le_bytes());
}

fn get_u16(src: &[u8]) -> u16 {
    u16::from_le_bytes([src[0], src[1]])
}

fn get_u32(src: &[u8]) -> u32 {
    u32::from_le_bytes([src[0], src[1], src[2], src[3]])
}

fn append_u32(b: &mut SecureBytes, v: u32) -> Result<(), VaultError> {
    b.extend_from_slice(&v.to_le_bytes())
        .map_err(|_| VaultError::CryptoError)
}

fn append_field(b: &mut SecureBytes, data: &[u8]) -> Result<(), VaultError> {
    append_u32(b, data.len() as u32)?;
    b.extend_from_slice(data)
        .map_err(|_| VaultError::CryptoError)
}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn read_u32(&mut self) -> Option<u32> {
        if self.b.len().saturating_sub(self.pos) < 4 {
            return None;
        }
        let v = get_u32(&self.b[self.pos..self.pos + 4]);
        self.pos += 4;
        Some(v)
    }

    fn read_field(&mut self) -> Option<&'a [u8]> {
        let len = self.read_u32()? as usize;
        if len > MAX_FIELD_LEN || self.b.len().saturating_sub(self.pos) < len {
            return None;
        }
        let slice = &self.b[self.pos..self.pos + len];
        self.pos += len;
        Some(slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::secure::init_process_memory;

    #[test]
    fn vault_create_unlock_v2_settings_and_password_rotation() {
        init_process_memory();

        let dir = std::env::temp_dir().join("orlogin_test_vault_v2");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let vault_path = dir.join("ORLogin.vault");

        let mut initial = VaultSettings::default();
        initial.key_delay_ms = 22;
        initial.auto_lock_minutes = 30;
        initial.input_method = InputMethod::SendInput;

        let mut v = Vault::new(vault_path.clone());
        v.create(b"MasterPass88", initial.clone()).unwrap();
        assert!(v.is_unlocked());

        v.accounts_mut().push(Account {
            label: "Main".to_owned(),
            username: "torq_user".to_owned(),
            password: ProtectedSecret::from_slice(b"Pass!@#123").unwrap(),
        });
        v.set_active_index(0);
        v.save().unwrap();
        v.lock();
        assert!(!v.is_unlocked());

        // Wrong password must fail
        let mut v2 = Vault::new(vault_path.clone());
        assert_eq!(v2.unlock(b"WrongPass99"), Err(VaultError::WrongPassword));

        // Correct password decrypts account + v2 settings
        v2.unlock(b"MasterPass88").unwrap();
        assert!(v2.has_loaded_settings());
        assert_eq!(v2.settings(), &initial);
        assert_eq!(v2.accounts().len(), 1);
        assert_eq!(v2.accounts()[0].username, "torq_user");
        assert_eq!(
            v2.accounts()[0].password.reveal().unwrap().as_slice(),
            b"Pass!@#123"
        );
        assert_eq!(v2.active_index(), 0);

        // Rotate master password: verify old password no longer unlocks vault OR .bak
        v2.change_master_password(b"MasterPass88", b"RotatedPass99")
            .unwrap();
        v2.lock();

        let mut v3 = Vault::new(vault_path.clone());
        assert_eq!(v3.unlock(b"MasterPass88"), Err(VaultError::WrongPassword));
        v3.unlock(b"RotatedPass99").unwrap();

        let bak_path = dir.join("ORLogin.vault.bak");
        assert!(bak_path.is_file());
        let mut v_bak = Vault::new(bak_path);
        assert_eq!(v_bak.unlock(b"MasterPass88"), Err(VaultError::WrongPassword));
        v_bak.unlock(b"RotatedPass99").unwrap();

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn vault_payload_v1_backward_compat_and_header_tamper() {
        init_process_memory();

        let dir = std::env::temp_dir().join("orlogin_test_vault_v1");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let vault_path = dir.join("ORLogin.vault");

        // Build a synthetic v1 payload ciphertext using the exact C++ v1 layout.
        let mut salt = [0u8; K_SALT_SIZE];
        let mut nonce = [0u8; K_NONCE_SIZE];
        random_bytes(&mut salt).unwrap();
        random_bytes(&mut nonce).unwrap();
        let iters = 100_000u32;

        let mut key_raw = [0u8; K_KEY_SIZE];
        pbkdf2_sha256(b"LegacyMaster8", &salt, iters, &mut key_raw).unwrap();

        let mut plain = SecureBytes::with_capacity(128).unwrap();
        append_u32(&mut plain, PAYLOAD_VERSION_1).unwrap();
        append_u32(&mut plain, 0).unwrap(); // active = 0
        append_u32(&mut plain, 1).unwrap(); // count = 1
        append_field(&mut plain, b"LegacyLabel").unwrap();
        append_field(&mut plain, b"legacy_id").unwrap();
        append_field(&mut plain, b"legacy_pw").unwrap();

        let mut out = vec![0u8; HEADER_SIZE + plain.len() + K_TAG_SIZE];
        out[0..4].copy_from_slice(&MAGIC);
        put_u16(&mut out[4..6], FORMAT_VERSION);
        put_u16(&mut out[6..8], KDF_PBKDF2);
        put_u32(&mut out[8..12], iters);
        out[12..28].copy_from_slice(&salt);
        out[28..40].copy_from_slice(&nonce);
        put_u32(&mut out[40..44], plain.len() as u32);

        let (aad, rest) = out.split_at_mut(HEADER_SIZE);
        let (ct, tag_out) = rest.split_at_mut(plain.len());
        let mut tag = [0u8; K_TAG_SIZE];
        aes_gcm_encrypt(&key_raw, &nonce, aad, plain.as_slice(), ct, &mut tag).unwrap();
        tag_out.copy_from_slice(&tag);
        fs::write(&vault_path, &out).unwrap();

        let mut v = Vault::new(vault_path.clone());
        v.unlock(b"LegacyMaster8").unwrap();
        assert!(!v.has_loaded_settings());
        assert_eq!(v.accounts().len(), 1);
        assert_eq!(v.accounts()[0].label, "LegacyLabel");
        assert_eq!(
            v.accounts()[0].password.reveal().unwrap().as_slice(),
            b"legacy_pw"
        );
        v.lock();

        // Tamper a byte in the nonce header (AAD) -> must fail authentication
        out[29] ^= 0xFF;
        fs::write(&vault_path, &out).unwrap();
        let mut v_tampered = Vault::new(vault_path);
        assert_eq!(
            v_tampered.unlock(b"LegacyMaster8"),
            Err(VaultError::WrongPassword)
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
