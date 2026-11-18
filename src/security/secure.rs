use std::ops::{Deref, DerefMut};
use std::ptr::{self, NonNull};
use std::slice;
use windows::Win32::Security::Cryptography::{
    CryptProtectMemory, CryptUnprotectMemory, CRYPTPROTECTMEMORY_BLOCK_SIZE,
    CRYPTPROTECTMEMORY_SAME_PROCESS,
};
use windows::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, VirtualLock, VirtualUnlock, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE,
    PAGE_READWRITE,
};
use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
use zeroize::Zeroize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecureError {
    AllocationFailed,
    ProtectionFailed,
}

/// Raises soft working-set limits so `VirtualLock` succeeds reliably for secret buffers.
pub fn init_process_memory() {
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), 8 * 1024 * 1024, 128 * 1024 * 1024);
    }
}

/// Wipes a slice in place using volatile writes that cannot be dead-store eliminated.
pub fn secure_wipe_slice(buf: &mut [u8]) {
    buf.zeroize();
}

/// Wipes a `String` in place and shrinks its capacity.
pub fn wipe_string(s: &mut String) {
    s.zeroize();
    s.shrink_to_fit();
}

/// Non-pageable, page-backed byte buffer (`VirtualAlloc` + best-effort `VirtualLock`)
/// that wipes its full allocation with volatile zeroization before releasing memory.
pub struct SecureBytes {
    ptr: NonNull<u8>,
    len: usize,
    cap: usize,
}

// `SecureBytes` uniquely owns its virtual-allocated buffer.
unsafe impl Send for SecureBytes {}
unsafe impl Sync for SecureBytes {}

impl SecureBytes {
    /// Allocates an empty buffer backed by a minimum 1-byte page allocation.
    pub fn empty() -> Result<Self, SecureError> {
        Self::with_capacity(0)
    }

    pub fn with_capacity(cap: usize) -> Result<Self, SecureError> {
        let alloc_bytes = cap.max(1);
        let raw = unsafe {
            VirtualAlloc(
                None,
                alloc_bytes,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        let Some(ptr) = NonNull::new(raw as *mut u8) else {
            return Err(SecureError::AllocationFailed);
        };
        unsafe {
            // Best-effort keep out of pagefile; non-fatal if quota is tight.
            let _ = VirtualLock(ptr.as_ptr() as *const _, alloc_bytes);
        }
        Ok(Self {
            ptr,
            len: 0,
            cap: alloc_bytes,
        })
    }

    pub fn zeroed(len: usize) -> Result<Self, SecureError> {
        let mut buf = Self::with_capacity(len)?;
        buf.len = len;
        // VirtualAlloc commits zeroed pages, explicitly clear for certainty.
        secure_wipe_slice(buf.as_mut_slice());
        Ok(buf)
    }

    pub fn from_slice(data: &[u8]) -> Result<Self, SecureError> {
        let mut buf = Self::with_capacity(data.len())?;
        if !data.is_empty() {
            unsafe {
                ptr::copy_nonoverlapping(data.as_ptr(), buf.ptr.as_ptr(), data.len());
            }
            buf.len = data.len();
        }
        Ok(buf)
    }

    /// Reads NUL-terminated bytes from an edit buffer into an exact-size `SecureBytes`.
    pub fn from_cstr_buffer(buf: &[u8]) -> Result<Self, SecureError> {
        let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        Self::from_slice(&buf[..len])
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn as_slice(&self) -> &[u8] {
        if self.len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
        }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        if self.len == 0 {
            &mut []
        } else {
            unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
        }
    }

    pub fn push_u8(&mut self, b: u8) -> Result<(), SecureError> {
        self.reserve(1)?;
        unsafe {
            *self.ptr.as_ptr().add(self.len) = b;
        }
        self.len += 1;
        Ok(())
    }

    pub fn extend_from_slice(&mut self, data: &[u8]) -> Result<(), SecureError> {
        if data.is_empty() {
            return Ok(());
        }
        self.reserve(data.len())?;
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), self.ptr.as_ptr().add(self.len), data.len());
        }
        self.len += data.len();
        Ok(())
    }

    pub fn reserve(&mut self, additional: usize) -> Result<(), SecureError> {
        let required = self.len.saturating_add(additional);
        if required <= self.cap {
            return Ok(());
        }
        let new_cap = required.max(self.cap.saturating_mul(2));
        let mut next = Self::with_capacity(new_cap)?;
        if self.len > 0 {
            unsafe {
                ptr::copy_nonoverlapping(self.ptr.as_ptr(), next.ptr.as_ptr(), self.len);
            }
            next.len = self.len;
        }
        *self = next;
        Ok(())
    }

    /// Shrinks logical length without leaking leftover tail bytes.
    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.len {
            let tail = unsafe {
                slice::from_raw_parts_mut(self.ptr.as_ptr().add(new_len), self.len - new_len)
            };
            secure_wipe_slice(tail);
            self.len = new_len;
        }
    }

    /// Wipes the entire allocated buffer in place while keeping `len` unchanged.
    pub fn wipe_in_place(&mut self) {
        let all = unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.cap) };
        secure_wipe_slice(all);
    }

    /// Constant-time byte comparison.
    pub fn ct_eq(&self, other: &Self) -> bool {
        let a = self.as_slice();
        let b = other.as_slice();
        let mut diff = (a.len() ^ b.len()) as u8;
        let min = a.len().min(b.len());
        for i in 0..min {
            diff |= a[i] ^ b[i];
        }
        diff == 0
    }
}

impl Deref for SecureBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl DerefMut for SecureBytes {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

impl Zeroize for SecureBytes {
    fn zeroize(&mut self) {
        self.wipe_in_place();
        self.len = 0;
    }
}

impl Drop for SecureBytes {
    fn drop(&mut self) {
        unsafe {
            let all = slice::from_raw_parts_mut(self.ptr.as_ptr(), self.cap);
            secure_wipe_slice(all);
            let _ = VirtualUnlock(self.ptr.as_ptr() as *const _, self.cap);
            let _ = VirtualFree(self.ptr.as_ptr() as *mut _, 0, MEM_RELEASE);
        }
    }
}

/// Secret held encrypted in RAM via DPAPI `CryptProtectMemory(SAME_PROCESS)`.
/// Only decrypted into a temporary `SecureBytes` buffer at the moment of use.
#[derive(Default)]
pub struct ProtectedSecret {
    blob: Option<SecureBytes>, // CRYPTPROTECTMEMORY_BLOCK_SIZE-padded ciphertext
    len: usize,
}

impl ProtectedSecret {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_slice(data: &[u8]) -> Result<Self, SecureError> {
        let mut s = Self::new();
        s.set(data)?;
        Ok(s)
    }

    /// Replaces contents, copying `data` into locked memory and encrypting in place.
    /// Never leaves an unencrypted copy around if protection fails.
    pub fn set(&mut self, data: &[u8]) -> Result<(), SecureError> {
        self.clear();
        if data.is_empty() {
            return Ok(());
        }

        let block = CRYPTPROTECTMEMORY_BLOCK_SIZE as usize;
        let padded = data.len().div_ceil(block) * block;
        let mut buf = SecureBytes::zeroed(padded)?;
        buf.as_mut_slice()[..data.len()].copy_from_slice(data);

        let res = unsafe {
            CryptProtectMemory(
                buf.as_mut_slice().as_mut_ptr() as *mut _,
                padded as u32,
                CRYPTPROTECTMEMORY_SAME_PROCESS,
            )
        };
        if res.is_err() {
            // `buf` wipes itself on drop.
            return Err(SecureError::ProtectionFailed);
        }

        self.blob = Some(buf);
        self.len = data.len();
        Ok(())
    }

    pub fn reveal(&self) -> Result<SecureBytes, SecureError> {
        if self.len == 0 {
            return SecureBytes::empty();
        }
        let Some(blob) = &self.blob else {
            return SecureBytes::empty();
        };

        let mut out = SecureBytes::from_slice(blob.as_slice())?;
        let res = unsafe {
            CryptUnprotectMemory(
                out.as_mut_slice().as_mut_ptr() as *mut _,
                out.len() as u32,
                CRYPTPROTECTMEMORY_SAME_PROCESS,
            )
        };
        if res.is_err() {
            return Err(SecureError::ProtectionFailed);
        }
        out.truncate(self.len);
        Ok(out)
    }

    pub fn clear(&mut self) {
        self.blob = None;
        self.len = 0;
    }

    pub fn size(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Zeroize for ProtectedSecret {
    fn zeroize(&mut self) {
        self.clear();
    }
}

impl Drop for ProtectedSecret {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_bytes_and_protected_secret_roundtrip() {
        init_process_memory();

        let raw = b"MySuperSecretROPassword!";
        let mut ps = ProtectedSecret::new();
        ps.set(raw).expect("CryptProtectMemory failed");
        assert_eq!(ps.size(), raw.len());

        let revealed = ps.reveal().expect("CryptUnprotectMemory failed");
        assert_eq!(revealed.as_slice(), raw);

        ps.clear();
        assert!(ps.is_empty());
        assert_eq!(ps.reveal().unwrap().len(), 0);
    }

    #[test]
    fn secure_bytes_ct_eq_and_truncate() {
        let mut a = SecureBytes::from_slice(b"abcdef").unwrap();
        let b = SecureBytes::from_slice(b"abc").unwrap();
        assert!(!a.ct_eq(&b));
        a.truncate(3);
        assert!(a.ct_eq(&b));
    }
}
