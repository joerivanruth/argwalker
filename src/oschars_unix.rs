use std::os::unix::ffi::OsStrExt;

use std::ffi::OsStr;
use std::str;

pub fn split_valid(s: &OsStr) -> (&str, &OsStr) {
    let bytes: &[u8] = OsStrExt::as_bytes(s);

    let valid_to = match str::from_utf8(bytes) {
        Ok(s) => s.len(),
        Err(e) => e.valid_up_to(),
    };

    let valid_head = unsafe {
        // SAFETY: valid_to was derived from std::from_utf8.
        str::from_utf8_unchecked(&bytes[..valid_to])
    };
    let invalid_tail = OsStrExt::from_bytes(&bytes[valid_to..]);

    (valid_head, invalid_tail)
}
