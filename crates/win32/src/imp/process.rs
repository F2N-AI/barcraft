//! One running copy, the crash watchdog, and starting with Windows.

use std::ffi::c_void;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, GetLastError, HANDLE,
};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::Win32::System::Threading::{
    CreateMutexW, INFINITE, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};
use windows::core::{PCWSTR, w};

use super::util::wide;

/// Holds a named mutex so that a second copy of the app can tell one is running.
#[derive(Debug)]
pub struct SingleInstance {
    handle: isize,
}

impl SingleInstance {
    /// `None` when another copy already holds `name`.
    pub fn acquire(name: &str) -> Option<SingleInstance> {
        let name = wide(name);
        // SAFETY: `name` is NUL-terminated; the handle is closed in Drop.
        let handle = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) }.ok()?;
        // SAFETY: reads this thread's last error, set by CreateMutexW just above.
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            // SAFETY: closing the handle we were just given.
            let _ = unsafe { CloseHandle(handle) };
            return None;
        }
        Some(SingleInstance {
            handle: handle.0 as isize,
        })
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateMutexW and is closed once.
        let _ = unsafe { CloseHandle(HANDLE(self.handle as *mut c_void)) };
    }
}

/// Blocks until process `pid` ends. Returns `false` if it couldn't be watched.
pub fn wait_for_exit(pid: u32) -> bool {
    // SAFETY: a wait-only handle, closed below.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }) else {
        return false;
    };
    // SAFETY: waiting on a handle we own.
    unsafe { WaitForSingleObject(handle, INFINITE) };
    // SAFETY: closed once.
    let _ = unsafe { CloseHandle(handle) };
    true
}

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

/// Adds (`Some(command)`) or removes (`None`) a per-user "start with Windows" entry.
pub fn set_launch_at_startup(name: &str, command: Option<&str>) -> bool {
    let value = wide(name);
    match command {
        Some(cmd) => {
            let data = wide(cmd);
            let Ok(bytes) = u32::try_from(data.len() * 2) else {
                return false;
            };
            // SAFETY: `data` is NUL-terminated UTF-16 of `bytes` bytes.
            let res = unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    RUN_KEY,
                    PCWSTR(value.as_ptr()),
                    REG_SZ.0,
                    Some(data.as_ptr() as *const c_void),
                    bytes,
                )
            };
            res == ERROR_SUCCESS
        }
        None => {
            // SAFETY: deletes one value under the user's Run key.
            let res =
                unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, PCWSTR(value.as_ptr())) };
            res == ERROR_SUCCESS || !launch_at_startup(name)
        }
    }
}

pub fn launch_at_startup(name: &str) -> bool {
    let value = wide(name);
    // SAFETY: only asks whether the value exists; no data buffer.
    let res = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        )
    };
    res == ERROR_SUCCESS
}
