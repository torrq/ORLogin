use crate::security::secure::secure_wipe_slice;
use windows::core::PCWSTR;
use windows::Win32::Foundation::STATUS_AUTH_TAG_MISMATCH;
use windows::Win32::Security::Cryptography::{
    BCryptCloseAlgorithmProvider, BCryptDecrypt, BCryptDeriveKeyPBKDF2, BCryptDestroyKey,
    BCryptEncrypt, BCryptGenRandom, BCryptGenerateSymmetricKey, BCryptOpenAlgorithmProvider,
    BCryptSetProperty, BCRYPT_AES_ALGORITHM, BCRYPT_ALG_HANDLE, BCRYPT_ALG_HANDLE_HMAC_FLAG,
    BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO, BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO_VERSION,
    BCRYPT_CHAINING_MODE, BCRYPT_CHAIN_MODE_GCM, BCRYPT_FLAGS, BCRYPT_KEY_HANDLE,
    BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS, BCRYPT_SHA256_ALGORITHM,
    BCRYPT_USE_SYSTEM_PREFERRED_RNG,
};

pub const K_KEY_SIZE: usize = 32; // AES-256
pub const K_SALT_SIZE: usize = 16;
pub const K_NONCE_SIZE: usize = 12; // GCM standard nonce
pub const K_TAG_SIZE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    AuthFailed,
    SystemError,
}

struct AlgGuard(BCRYPT_ALG_HANDLE);
impl Drop for AlgGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = BCryptCloseAlgorithmProvider(self.0, 0);
            }
        }
    }
}

struct KeyGuard(BCRYPT_KEY_HANDLE);
impl Drop for KeyGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = BCryptDestroyKey(self.0);
            }
        }
    }
}

fn open_aes_gcm(key_bytes: &[u8; K_KEY_SIZE]) -> Result<(AlgGuard, KeyGuard), CryptoError> {
    let mut h_alg = BCRYPT_ALG_HANDLE::default();
    let st = unsafe {
        BCryptOpenAlgorithmProvider(
            &mut h_alg,
            BCRYPT_AES_ALGORITHM,
            None,
            BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0),
        )
    };
    if st.is_err() {
        return Err(CryptoError::SystemError);
    }
    let alg = AlgGuard(h_alg);

    // BCRYPT_CHAIN_MODE_GCM is a wide null-terminated string literal in `windows`;
    // convert to byte slice including trailing NUL UTF-16 code unit for BCryptSetProperty.
    let gcm_mode_bytes = unsafe { slice_from_pcwstr_bytes(BCRYPT_CHAIN_MODE_GCM) };
    let st = unsafe { BCryptSetProperty(alg.0.into(), BCRYPT_CHAINING_MODE, gcm_mode_bytes, 0) };
    if st.is_err() {
        return Err(CryptoError::SystemError);
    }

    let mut h_key = BCRYPT_KEY_HANDLE::default();
    let st = unsafe { BCryptGenerateSymmetricKey(alg.0, &mut h_key, None, key_bytes, 0) };
    if st.is_err() {
        return Err(CryptoError::SystemError);
    }
    Ok((alg, KeyGuard(h_key)))
}

unsafe fn slice_from_pcwstr_bytes<'a>(p: PCWSTR) -> &'a [u8] {
    let mut len = 0usize;
    while *p.0.add(len) != 0 {
        len += 1;
    }
    std::slice::from_raw_parts(p.0 as *const u8, (len + 1) * 2)
}

pub fn random_bytes(out: &mut [u8]) -> Result<(), CryptoError> {
    if out.is_empty() {
        return Ok(());
    }
    let st = unsafe { BCryptGenRandom(None, out, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if st.is_err() {
        Err(CryptoError::SystemError)
    } else {
        Ok(())
    }
}

pub fn pbkdf2_sha256(
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    out_key: &mut [u8],
) -> Result<(), CryptoError> {
    let mut h_alg = BCRYPT_ALG_HANDLE::default();
    let st = unsafe {
        BCryptOpenAlgorithmProvider(
            &mut h_alg,
            BCRYPT_SHA256_ALGORITHM,
            None,
            BCRYPT_ALG_HANDLE_HMAC_FLAG,
        )
    };
    if st.is_err() {
        return Err(CryptoError::SystemError);
    }
    let alg = AlgGuard(h_alg);

    // Passing `&[]` to `Some(password)` gives a valid non-null slice pointer cleanly.
    let st = unsafe {
        BCryptDeriveKeyPBKDF2(
            alg.0,
            Some(password),
            Some(salt),
            iterations as u64,
            out_key,
            0,
        )
    };
    if st.is_err() {
        secure_wipe_slice(out_key);
        Err(CryptoError::SystemError)
    } else {
        Ok(())
    }
}

pub fn aes_gcm_encrypt(
    key: &[u8; K_KEY_SIZE],
    nonce: &[u8; K_NONCE_SIZE],
    aad: &[u8],
    plaintext: &[u8],
    ciphertext: &mut [u8],
    tag: &mut [u8; K_TAG_SIZE],
) -> Result<(), CryptoError> {
    if ciphertext.len() != plaintext.len() {
        return Err(CryptoError::SystemError);
    }
    let (_alg, h_key) = open_aes_gcm(key)?;

    let mut info = BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
        cbSize: std::mem::size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() as u32,
        dwInfoVersion: BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO_VERSION,
        pbNonce: nonce.as_ptr() as *mut u8,
        cbNonce: nonce.len() as u32,
        pbAuthData: if aad.is_empty() {
            std::ptr::null_mut()
        } else {
            aad.as_ptr() as *mut u8
        },
        cbAuthData: aad.len() as u32,
        pbTag: tag.as_mut_ptr(),
        cbTag: tag.len() as u32,
        ..Default::default()
    };

    let mut written = 0u32;
    let st = unsafe {
        BCryptEncrypt(
            h_key.0,
            Some(plaintext),
            Some(&mut info as *mut _ as *mut _),
            None,
            Some(ciphertext),
            &mut written,
            BCRYPT_FLAGS(0),
        )
    };
    if st.is_err() || written as usize != plaintext.len() {
        Err(CryptoError::SystemError)
    } else {
        Ok(())
    }
}

pub fn aes_gcm_decrypt(
    key: &[u8; K_KEY_SIZE],
    nonce: &[u8; K_NONCE_SIZE],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; K_TAG_SIZE],
    plaintext: &mut [u8],
) -> Result<(), CryptoError> {
    if plaintext.len() != ciphertext.len() {
        return Err(CryptoError::SystemError);
    }
    let (_alg, h_key) = open_aes_gcm(key)?;

    let mut info = BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
        cbSize: std::mem::size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() as u32,
        dwInfoVersion: BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO_VERSION,
        pbNonce: nonce.as_ptr() as *mut u8,
        cbNonce: nonce.len() as u32,
        pbAuthData: if aad.is_empty() {
            std::ptr::null_mut()
        } else {
            aad.as_ptr() as *mut u8
        },
        cbAuthData: aad.len() as u32,
        pbTag: tag.as_ptr() as *mut u8,
        cbTag: tag.len() as u32,
        ..Default::default()
    };

    let mut written = 0u32;
    let st = unsafe {
        BCryptDecrypt(
            h_key.0,
            Some(ciphertext),
            Some(&mut info as *mut _ as *mut _),
            None,
            Some(plaintext),
            &mut written,
            BCRYPT_FLAGS(0),
        )
    };
    if st == STATUS_AUTH_TAG_MISMATCH {
        secure_wipe_slice(plaintext);
        return Err(CryptoError::AuthFailed);
    }
    if st.is_err() || written as usize != ciphertext.len() {
        secure_wipe_slice(plaintext);
        return Err(CryptoError::SystemError);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cng_aes_gcm_and_pbkdf2_roundtrip() {
        let mut salt = [0u8; K_SALT_SIZE];
        let mut nonce = [0u8; K_NONCE_SIZE];
        random_bytes(&mut salt).unwrap();
        random_bytes(&mut nonce).unwrap();

        let mut key = [0u8; K_KEY_SIZE];
        pbkdf2_sha256(b"password123", &salt, 1000, &mut key).unwrap();

        let aad = b"ORLV_HEADER";
        let pt = b"secret payload";
        let mut ct = vec![0u8; pt.len()];
        let mut tag = [0u8; K_TAG_SIZE];
        aes_gcm_encrypt(&key, &nonce, aad, pt, &mut ct, &mut tag).unwrap();

        let mut out = vec![0u8; ct.len()];
        aes_gcm_decrypt(&key, &nonce, aad, &ct, &tag, &mut out).unwrap();
        assert_eq!(&out, pt);

        // Tamper AAD -> must return AuthFailed and wipe `out`
        let mut bad_aad = *aad;
        bad_aad[0] ^= 1;
        assert_eq!(
            aes_gcm_decrypt(&key, &nonce, &bad_aad, &ct, &tag, &mut out),
            Err(CryptoError::AuthFailed)
        );
        assert!(out.iter().all(|&b| b == 0));
    }
}
