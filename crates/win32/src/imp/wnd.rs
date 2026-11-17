//! Finding, activating and closing the windows a taskbar would show.

use std::ffi::c_void;
use std::path::PathBuf;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    QueryFullProcessImageNameW, TerminateProcess,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GW_OWNER, GWL_EXSTYLE, GetClassNameW, GetForegroundWindow,
    GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, IsZoomed, PostMessageW,
    SW_MINIMIZE, SW_RESTORE, SetForegroundWindow, ShowWindow, SwitchToThisWindow, WM_CLOSE,
    WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PWSTR};

use super::appinfo::app_user_model_id;
use super::util::{hwnd, id_of, path_from_wide, string_from_wide};
use crate::{PxRect, WindowId, WindowInfo};

/// Classes that belong to the desktop and the shell, never to an app.
const SHELL_CLASSES: [&str; 5] = [
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Windows.UI.Core.CoreWindow",
];

/// The most windows we look at in one pass; a real desktop has a few hundred.
const MAX_WINDOWS: usize = 4096;

unsafe extern "system" fn collect(h: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the address of the Vec passed by `top_level_windows` /
    // `child_windows`, which outlives the enumeration and is only touched here.
    let list = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
    if list.len() < MAX_WINDOWS {
        list.push(h);
    }
    BOOL(1)
}

fn top_level_windows() -> Vec<HWND> {
    let mut list: Vec<HWND> = Vec::new();
    // SAFETY: the callback only pushes into `list`, which lives until the call returns.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut list as *mut Vec<HWND> as isize)) };
    list
}

fn child_windows(parent: HWND) -> Vec<HWND> {
    let mut list: Vec<HWND> = Vec::new();
    // SAFETY: as in `top_level_windows`.
    let _ = unsafe {
        EnumChildWindows(
            Some(parent),
            Some(collect),
            LPARAM(&mut list as *mut Vec<HWND> as isize),
        )
    };
    list
}

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: the buffer is valid for its whole length.
    let n = unsafe { GetClassNameW(h, &mut buf) };
    if n <= 0 {
        String::new()
    } else {
        string_from_wide(&buf)
    }
}

fn title(h: HWND) -> String {
    // SAFETY: plain query on a window handle; a stale handle returns 0.
    let len = unsafe { GetWindowTextLengthW(h) };
    if len <= 0 {
        return String::new();
    }
    let len = usize::try_from(len).unwrap_or(0).min(4096);
    let mut buf = vec![0u16; len + 1];
    // SAFETY: the buffer is valid for its whole length.
    let n = unsafe { GetWindowTextW(h, &mut buf) };
    if n <= 0 {
        String::new()
    } else {
        string_from_wide(&buf)
    }
}

fn pid_of(h: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid out pointer for the duration of the call.
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid
}

fn is_cloaked(h: HWND) -> bool {
    let mut cloaked = 0u32;
    // SAFETY: the out buffer is a u32, the size DWMWA_CLOAKED writes.
    let res = unsafe {
        DwmGetWindowAttribute(
            h,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        )
    };
    res.is_ok() && cloaked != 0
}

/// Whether a top-level window is one a taskbar would show a button for.
fn shows_on_taskbar(h: HWND) -> bool {
    // SAFETY: read-only queries on a handle from EnumWindows; stale handles just fail.
    let visible = unsafe { IsWindowVisible(h) }.as_bool();
    if !visible {
        return false;
    }
    // SAFETY: as above.
    let ex = unsafe { GetWindowLongPtrW(h, GWL_EXSTYLE) } as u32;
    let app_window = ex & WS_EX_APPWINDOW.0 != 0;
    if !app_window {
        if ex & WS_EX_TOOLWINDOW.0 != 0 || ex & WS_EX_NOACTIVATE.0 != 0 {
            return false;
        }
        // SAFETY: as above.
        let owner = unsafe { GetWindow(h, GW_OWNER) };
        if matches!(owner, Ok(o) if !o.is_invalid()) {
            return false;
        }
    }
    if is_cloaked(h) {
        return false;
    }
    let class = class_name(h);
    !SHELL_CLASSES.contains(&class.as_str())
}

/// The program file of a process, when the process lets us read it.
pub fn process_exe(pid: u32) -> Option<PathBuf> {
    process_identity(pid).0
}

/// The program file of a process and, for a packaged (Store) app, the app's AppUserModelID.
fn process_identity(pid: u32) -> (Option<PathBuf>, Option<String>) {
    if pid == 0 {
        return (None, None);
    }
    // SAFETY: requesting a query-only handle; closed below.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return (None, None);
    };
    let exe = image_path(handle);
    let app_id = app_user_model_id(handle);
    // SAFETY: `handle` came from OpenProcess and is closed exactly once.
    let _ = unsafe { CloseHandle(handle) };
    (exe, app_id)
}

/// The program file behind a process handle opened with query rights.
fn image_path(handle: HANDLE) -> Option<PathBuf> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` and `len` describe the same writable buffer.
    let res = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    };
    res.ok()?;
    let path = path_from_wide(&buf);
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

/// Store apps run inside a frame host process; the app's own process owns a child window.
fn hosted_app_pid(frame: HWND, frame_pid: u32) -> Option<u32> {
    child_windows(frame)
        .into_iter()
        .map(pid_of)
        .find(|&pid| pid != 0 && pid != frame_pid)
}

/// The windows a taskbar would list, top of the stacking order first.
/// Windows of `exclude_pid` (BarCraft itself) are left out.
pub fn list_app_windows(exclude_pid: u32) -> Vec<WindowInfo> {
    let mut out = Vec::new();
    for h in top_level_windows() {
        if !shows_on_taskbar(h) {
            continue;
        }
        let pid = pid_of(h);
        if pid == exclude_pid {
            continue;
        }
        let title = title(h);
        if title.is_empty() {
            continue;
        }
        let (mut exe, mut app_id) = process_identity(pid);
        let is_frame_host = exe
            .as_deref()
            .and_then(|p| p.file_name())
            .is_some_and(|n| n.eq_ignore_ascii_case("ApplicationFrameHost.exe"));
        if is_frame_host && let Some(hosted) = hosted_app_pid(h, pid) {
            // The app is the hosted process, not the frame host.
            let (hosted_exe, hosted_id) = process_identity(hosted);
            exe = hosted_exe.or(exe);
            app_id = hosted_id;
        }
        // SAFETY: read-only query on a live handle.
        let minimized = unsafe { IsIconic(h) }.as_bool();
        out.push(WindowInfo {
            id: id_of(h),
            title,
            pid,
            exe,
            minimized,
            app_id,
            app_name: None,
        });
    }
    out
}

pub fn foreground_window() -> Option<WindowId> {
    // SAFETY: no arguments; returns null when no window has focus.
    let h = unsafe { GetForegroundWindow() };
    if h.is_invalid() { None } else { Some(id_of(h)) }
}

/// Whether the focused window is part of the shell's own taskbar UI: the taskbar, its tray
/// and the tray's pop-ups (not File Explorer windows, which share the shell's process).
pub fn foreground_is_shell_ui() -> bool {
    // SAFETY: no arguments.
    let h = unsafe { GetForegroundWindow() };
    if h.is_invalid() {
        return false;
    }
    let class = class_name(h);
    if matches!(
        class.as_str(),
        "CabinetWClass" | "ExploreWClass" | "Progman" | "WorkerW"
    ) {
        return false;
    }
    let exe = process_exe(pid_of(h));
    let name = exe
        .as_deref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    class.starts_with("Shell_")
        || matches!(name.as_str(), "explorer.exe" | "shellexperiencehost.exe")
}

pub fn is_window(id: WindowId) -> bool {
    // SAFETY: IsWindow accepts any value and reports whether it names a window.
    unsafe { IsWindow(Some(hwnd(id))) }.as_bool()
}

/// Brings a window to the front, restoring it if minimised.
pub fn activate_window(id: WindowId) {
    let h = hwnd(id);
    if !is_window(id) {
        return;
    }
    // SAFETY: the handle names a window; these calls only change its show state and focus.
    unsafe {
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        if !SetForegroundWindow(h).as_bool() {
            // Windows can refuse focus changes; this asks the shell's way instead.
            SwitchToThisWindow(h, true);
        }
    }
}

pub fn minimize_window(id: WindowId) {
    if !is_window(id) {
        return;
    }
    // SAFETY: the handle names a window; minimising has no other effect.
    let _ = unsafe { ShowWindow(hwnd(id), SW_MINIMIZE) };
}

/// Asks a window to close, as its close button would.
pub fn close_window(id: WindowId) {
    if !is_window(id) {
        return;
    }
    // SAFETY: posting WM_CLOSE with no pointers attached.
    let _ = unsafe {
        PostMessageW(
            Some(hwnd(id)),
            WM_CLOSE,
            Default::default(),
            Default::default(),
        )
    };
}

/// Ends a process at once. Returns whether it worked.
pub fn end_process(pid: u32) -> bool {
    if pid == 0 || pid == std::process::id() {
        return false;
    }
    // SAFETY: the handle is closed below on every path.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }) else {
        return false;
    };
    // SAFETY: `handle` has PROCESS_TERMINATE access.
    let ok = unsafe { TerminateProcess(handle, 1) }.is_ok();
    // SAFETY: closed exactly once.
    let _ = unsafe { CloseHandle(handle) };
    ok
}

fn window_rect(h: HWND) -> Option<PxRect> {
    let mut r = RECT::default();
    // SAFETY: `r` is a valid out pointer.
    unsafe { GetWindowRect(h, &mut r) }.ok()?;
    Some(PxRect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    })
}

/// Whether the focused window covers the whole of `monitor`, as a game or a full-screen video does.
pub fn foreground_is_fullscreen(own: WindowId, monitor: PxRect) -> bool {
    // SAFETY: no arguments.
    let h = unsafe { GetForegroundWindow() };
    if h.is_invalid() || id_of(h) == own {
        return false;
    }
    let class = class_name(h);
    if SHELL_CLASSES.contains(&class.as_str()) {
        return false;
    }
    // A maximised window also covers the whole screen when the dock reserves no space (it
    // reaches a little past the edges), but it isn't full-screen: the dock stays.
    // SAFETY: read-only query on a live handle.
    if unsafe { IsZoomed(h) }.as_bool() {
        return false;
    }
    match window_rect(h) {
        Some(r) => {
            r.left <= monitor.left
                && r.top <= monitor.top
                && r.right >= monitor.right
                && r.bottom >= monitor.bottom
        }
        None => false,
    }
}
