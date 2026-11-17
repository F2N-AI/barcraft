//! Small conversions shared by the Win32 modules.

use std::ffi::{OsStr, OsString, c_void};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};

use crate::WindowId;

/// NUL-terminated UTF-16 copy of a string.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// NUL-terminated UTF-16 copy of an OS string (paths keep unpaired surrogates).
pub(crate) fn wide_os(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// The text before the first NUL.
pub(crate) fn until_nul(buf: &[u16]) -> &[u16] {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    buf.get(..len).unwrap_or(&[])
}

pub(crate) fn string_from_wide(buf: &[u16]) -> String {
    String::from_utf16_lossy(until_nul(buf))
}

pub(crate) fn path_from_wide(buf: &[u16]) -> PathBuf {
    PathBuf::from(OsString::from_wide(until_nul(buf)))
}

pub(crate) fn hwnd(id: WindowId) -> HWND {
    HWND(id.0 as *mut c_void)
}

pub(crate) fn id_of(h: HWND) -> WindowId {
    WindowId(h.0 as isize)
}

/// Replaces `%NAME%` with the environment variable's value; unknown names stay as written.
pub(crate) fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        let (before, after) = rest.split_at(start);
        out.push_str(before);
        let after = after.get(1..).unwrap_or("");
        match after.find('%') {
            Some(end) => {
                let name = after.get(..end).unwrap_or("");
                match std::env::var(name) {
                    Ok(value) if !name.is_empty() => out.push_str(&value),
                    _ => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = after.get(end + 1..).unwrap_or("");
            }
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// COM for the current thread, released when dropped if this call started it.
pub(crate) struct Com {
    started: bool,
}

impl Com {
    pub(crate) fn init() -> Com {
        // SAFETY: no reserved pointer; balanced by CoUninitialize in Drop when it succeeded
        // (S_OK and S_FALSE both count and both need the matching call).
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Com {
            started: hr.is_ok(),
        }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.started {
            // SAFETY: matches the successful CoInitializeEx on this thread.
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_known_variables_only() {
        // SAFETY of the test: single-threaded test sets its own variable.
        let home = std::env::var("SystemRoot").unwrap_or_default();
        assert_eq!(expand_env("%SystemRoot%\\x.dll"), format!("{home}\\x.dll"));
        assert_eq!(expand_env("%NoSuchVarHere%\\x"), "%NoSuchVarHere%\\x");
        assert_eq!(expand_env("100% sure"), "100% sure");
        assert_eq!(expand_env(""), "");
    }

    #[test]
    fn wide_round_trip() {
        let w = wide("Città");
        assert_eq!(w.last(), Some(&0));
        assert_eq!(string_from_wide(&w), "Città");
        assert_eq!(string_from_wide(&[]), "");
    }
}
