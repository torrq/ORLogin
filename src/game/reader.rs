use encoding_rs::WINDOWS_1252;
use std::ffi::c_void;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    PROCESS_VM_READ,
};

/// Read-only access to an external game client's virtual memory.
pub struct ProcessReader {
    handle: HANDLE,
    pid: u32,
    can_sync: bool,
}

unsafe impl Send for ProcessReader {}

impl Drop for ProcessReader {
    fn drop(&mut self) {
        self.close();
    }
}

impl ProcessReader {
    pub fn open(pid: u32) -> Option<Self> {
        // Try richer set first (allows WaitForSingleObject exit detection), fall back to VM_READ.
        let (handle, can_sync) = unsafe {
            match OpenProcess(
                PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                false,
                pid,
            ) {
                Ok(h) if !h.is_invalid() => (h, true),
                _ => match OpenProcess(PROCESS_VM_READ, false, pid) {
                    Ok(h) if !h.is_invalid() => (h, false),
                    _ => return None,
                },
            }
        };
        Some(Self {
            handle,
            pid,
            can_sync,
        })
    }

    pub fn close(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
            self.handle = HANDLE::default();
        }
        self.pid = 0;
        self.can_sync = false;
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn has_exited(&self) -> bool {
        if self.handle.is_invalid() {
            return true;
        }
        if !self.can_sync {
            return false;
        }
        unsafe { WaitForSingleObject(self.handle, 0) == WAIT_OBJECT_0 }
    }

    pub fn read_exact(&self, address: u32, out: &mut [u8]) -> bool {
        if self.handle.is_invalid() || address == 0 || out.is_empty() {
            return false;
        }
        let mut bytes_read = 0usize;
        let res = unsafe {
            ReadProcessMemory(
                self.handle,
                address as usize as *const c_void,
                out.as_mut_ptr() as *mut c_void,
                out.len(),
                Some(&mut bytes_read),
            )
        };
        res.is_ok() && bytes_read == out.len()
    }

    pub fn read_u32(&self, address: u32) -> Option<u32> {
        let mut buf = [0u8; 4];
        if self.read_exact(address, &mut buf) {
            Some(u32::from_le_bytes(buf))
        } else {
            None
        }
    }

    pub fn read_u8(&self, address: u32) -> Option<u8> {
        let mut buf = [0u8; 1];
        if self.read_exact(address, &mut buf) {
            Some(buf[0])
        } else {
            None
        }
    }

    /// Reads a NUL-terminated CP-1252 string up to `max_len` bytes, returned as UTF-8.
    pub fn read_cp1252_string(&self, address: u32, max_len: usize) -> String {
        if max_len == 0 {
            return String::new();
        }
        let mut buf = vec![0u8; max_len];
        if !self.read_exact(address, &mut buf) {
            return String::new();
        }
        decode_cp1252_cstr(&buf)
    }
}

pub fn decode_cp1252_cstr(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    if end == 0 {
        return String::new();
    }
    let (decoded, _, _) = WINDOWS_1252.decode(&buf[..end]);
    decoded.into_owned()
}

#[cfg(test)]
mod tests {
    use super::decode_cp1252_cstr;

    #[test]
    fn cp1252_decodes_ro_trademark_glyph() {
        let raw = b"Torq\x99\0junk";
        assert_eq!(decode_cp1252_cstr(raw), "Torq\u{2122}");
    }
}
