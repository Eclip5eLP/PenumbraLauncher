//! Windows Credential Manager: the only place the refresh token is stored (never in files or logs).
//! Visible to the user under "Windows Credentials" as `Penumbra Launcher/refresh_token`.

use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::Security::Credentials::{
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW, CredWriteW,
};

const TARGET: &str = "Penumbra Launcher/refresh_token";

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

pub fn save(secret: &str) -> bool {
    let target = wide(TARGET);
    let user = wide("Penumbra Account");
    let blob = secret.as_bytes();
    let cred = CREDENTIALW {
        Flags: 0,
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_ptr() as *mut u16,
        Comment: std::ptr::null_mut(),
        LastWritten: unsafe { std::mem::zeroed() },
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_ptr() as *mut u8,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: std::ptr::null_mut(),
        TargetAlias: std::ptr::null_mut(),
        UserName: user.as_ptr() as *mut u16,
    };
    // SAFETY: all pointers reference live buffers for the duration of the call; the API copies the data.
    unsafe { CredWriteW(&cred, 0) != 0 }
}

pub fn load() -> Option<String> {
    let target = wide(TARGET);
    let mut ptr: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: valid target string; on success `ptr` points to a buffer we free with CredFree.
    if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut ptr) } == 0 || ptr.is_null() {
        return None;
    }
    // SAFETY: CredReadW returned a valid CREDENTIALW whose blob has CredentialBlobSize bytes.
    let secret = unsafe {
        let c = &*ptr;
        let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
        let s = String::from_utf8(bytes.to_vec()).ok();
        CredFree(ptr as *const _);
        s
    };
    secret.filter(|s| !s.is_empty())
}

pub fn delete() {
    let target = wide(TARGET);
    // SAFETY: valid target string; deleting a missing credential just fails.
    unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) };
}
