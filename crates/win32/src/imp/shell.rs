//! Opening programs, shortcuts and the Start menu.

use std::path::{Path, PathBuf};

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_LWIN,
};
use windows::Win32::UI::Shell::{IShellLinkW, SLGP_RAWPATH, ShellExecuteW, ShellLink};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{IUnknown, Interface, PCWSTR, w};

use super::util::{Com, expand_env, string_from_wide, wide, wide_os};
use crate::{Shortcut, TaskbarPin};

/// The most pinned shortcuts we read.
const MAX_PINS: usize = 64;

/// Reads a `.lnk` file's target and icon.
pub fn resolve_shortcut(path: &Path) -> Option<Shortcut> {
    let _com = Com::init();
    let file = wide_os(path.as_os_str());
    // SAFETY: standard COM creation; the interfaces are released when dropped.
    let link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None::<&IUnknown>, CLSCTX_INPROC_SERVER) }.ok()?;
    let persist: IPersistFile = link.cast().ok()?;
    // SAFETY: `file` is NUL-terminated and outlives the call.
    unsafe { persist.Load(PCWSTR(file.as_ptr()), STGM_READ) }.ok()?;

    let mut buf = [0u16; 1024];
    // SAFETY: `buf` is writable for its whole length; find-data is optional and not wanted.
    let target = unsafe { link.GetPath(&mut buf, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32) }
        .ok()
        .map(|_| expand_env(&string_from_wide(&buf)))
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    let mut icon_buf = [0u16; 1024];
    let mut index = 0i32;
    // SAFETY: as above; `index` is a valid out pointer.
    let icon = unsafe { link.GetIconLocation(&mut icon_buf, &mut index) }
        .ok()
        .map(|_| expand_env(&string_from_wide(&icon_buf)))
        .filter(|s| !s.is_empty())
        .map(|s| (PathBuf::from(s), index));

    Some(Shortcut { target, icon })
}

/// The shortcuts pinned to the system taskbar, sorted by name.
pub fn taskbar_pins() -> Vec<TaskbarPin> {
    let Ok(appdata) = std::env::var("APPDATA") else {
        return Vec::new();
    };
    let dir = Path::new(&appdata)
        .join("Microsoft")
        .join("Internet Explorer")
        .join("Quick Launch")
        .join("User Pinned")
        .join("TaskBar");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut pins: Vec<TaskbarPin> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")))
        .take(MAX_PINS)
        .map(|link| {
            let name = link
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let resolved = resolve_shortcut(&link).unwrap_or_default();
            // Some shell shortcuts (File Explorer, for one) point at a shell location rather
            // than a file; their icon usually comes from the program that opens it.
            let icon_program = resolved
                .icon
                .map(|(file, _)| file)
                .filter(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")));
            let target = resolved.target.or(icon_program);
            TaskbarPin { link, name, target }
        })
        .collect();
    pins.sort_by_key(|p| p.name.to_lowercase());
    pins
}

fn shell_open(file: &[u16]) -> bool {
    // SAFETY: `file` is NUL-terminated and outlives the call; no parameters or directory.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecute reports success with any value above 32.
    result.0 as isize > 32
}

/// Opens a program, shortcut or document the way a double-click would.
pub fn launch(path: &Path) -> bool {
    shell_open(&wide_os(path.as_os_str()))
}

/// Opens an https link in the default browser. Other schemes are refused.
pub fn open_url(url: &str) -> bool {
    if !url.starts_with("https://") {
        return false;
    }
    shell_open(&wide(url))
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(keys: &[INPUT]) {
    // SAFETY: `keys` is a valid slice of keyboard inputs and the size is that of INPUT.
    unsafe { SendInput(keys, std::mem::size_of::<INPUT>() as i32) };
}

/// Opens the Start menu, as the Windows key does.
pub fn open_start_menu() {
    send(&[key(VK_LWIN, false), key(VK_LWIN, true)]);
}

/// The Windows key together with a letter key.
fn win_plus(letter: u8) {
    let k = VIRTUAL_KEY(u16::from(letter));
    send(&[
        key(VK_LWIN, false),
        key(k, false),
        key(k, true),
        key(VK_LWIN, true),
    ]);
}

/// Minimises every window, or brings them back (the Windows key + D).
pub fn toggle_desktop() {
    win_plus(b'D');
}

/// Opens the notification centre (the Windows key + N).
pub fn open_notification_center() {
    win_plus(b'N');
}

/// Puts keyboard focus on the system tray (the Windows key + B), which also brings a hidden
/// or auto-hidden taskbar into view.
pub fn focus_system_tray() {
    win_plus(b'B');
}

/// Opens the Recycle Bin folder.
pub fn open_recycle_bin() -> bool {
    shell_open(&wide("shell:RecycleBinFolder"))
}
