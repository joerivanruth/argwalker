
use std::ffi::{OsStr, OsString};
use std::str;

#[cfg(unix)]
fn to_bytes(s: &OsStr) -> &[u8] {
    std::os::unix::ffi::OsStrExt::as_bytes(s)
}

#[cfg(unix)]
fn from_bytes(bytes: &[u8]) -> &OsStr {
    std::os::unix::ffi::OsStrExt::from_bytes(bytes)
}

#[cfg(not(unix))]
fn to_bytes(s: &OsStr) -> &[u8] {
    let _ = s;
    unimplemented!("to_bytes only implemented on Unix")
}

#[cfg(not(unix))]
fn from_bytes(bytes: &[u8]) -> &OsStr {
    let _ = bytes;
    unimplemented!("from_bytes only implemented on Unix")
}
#[allow(dead_code)]
pub fn split_valid(s: &OsStr) -> (String, OsString) {
    let bytes = to_bytes(s);

    let valid_to = match str::from_utf8(bytes) {
        Ok(s) => s.len(),
        Err(e) => e.valid_up_to(),
    };

    let valid_head = unsafe {
        // SAFETY: valid_to was derived from std::from_utf8.
        str::from_utf8_unchecked(&bytes[..valid_to])
    };
    let invalid_tail = from_bytes(&bytes[valid_to..]);

    (valid_head.to_string(), invalid_tail.to_os_string())
}

#[allow(dead_code)]
pub fn bad_text(prefix: &str) -> OsString {
    let mut s = OsString::from(prefix);
    s.push(from_bytes(&[0xFF]).to_os_string());
    s
}
