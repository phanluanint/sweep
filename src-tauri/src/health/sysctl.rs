//! Small typed wrappers over `sysctlbyname`.

use std::ffi::CString;

fn raw(name: &str, buf: &mut [u8]) -> Option<usize> {
    let cname = CString::new(name).ok()?;
    let mut len = buf.len();
    // SAFETY: buf is valid for `len` bytes; sysctlbyname writes at most `len` and updates it.
    let rc = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            buf.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (rc == 0).then_some(len)
}

/// Reads a 32- or 64-bit integer value.
pub fn int(name: &str) -> Option<i64> {
    let mut buf = [0u8; 8];
    match raw(name, &mut buf)? {
        4 => Some(i32::from_ne_bytes(buf[..4].try_into().ok()?) as i64),
        8 => Some(i64::from_ne_bytes(buf)),
        _ => None,
    }
}

pub fn string(name: &str) -> Option<String> {
    let mut buf = [0u8; 256];
    let len = raw(name, &mut buf)?;
    let bytes = &buf[..len];
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    Some(String::from_utf8_lossy(&bytes[..end]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_known_values() {
        assert!(int("hw.ncpu").unwrap() > 0);
        assert!(int("hw.memsize").unwrap() > 0);
        assert!(!string("kern.ostype").unwrap().is_empty());
        assert_eq!(int("no.such.key"), None);
    }
}
