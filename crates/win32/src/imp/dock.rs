//! Placing the dock on a screen edge, reserving its space, and hiding the system taskbar.

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateRectRgn, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
    MONITORINFO, MonitorFromPoint, MonitorFromWindow, SetWindowRgn,
};
use windows::Win32::UI::Shell::{
    ABE_BOTTOM, ABE_TOP, ABM_GETSTATE, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, ABM_SETSTATE,
    ABS_ALWAYSONTOP, ABS_AUTOHIDE, APPBARDATA, DefSubclassProc, RemoveWindowSubclass,
    SHAppBarMessage, SetWindowSubclass,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetCursorPos, GetWindowLongPtrW,
    HWND_TOPMOST, IsWindowVisible, MA_NOACTIVATE, STYLESTRUCT, SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, WM_APP, WM_MOUSEACTIVATE, WM_NCDESTROY, WM_STYLECHANGING, WS_CAPTION,
    WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
};
use windows::core::{PCWSTR, w};

use super::util::hwnd;
use crate::{Edge, PxRect, WindowId};

/// Message the shell sends the dock about appbar changes. Nothing needs to answer it.
const APPBAR_CALLBACK: u32 = WM_APP + 0x42;

fn rect_of(r: RECT) -> PxRect {
    PxRect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    }
}

fn appbar_data(h: Option<HWND>) -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: h.unwrap_or_default(),
        ..Default::default()
    }
}

fn monitor_rect(monitor: windows::Win32::Graphics::Gdi::HMONITOR) -> Option<PxRect> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a MONITORINFO with its size set.
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        Some(rect_of(info.rcMonitor))
    } else {
        None
    }
}

/// The whole area of the main screen.
pub fn primary_monitor() -> Option<PxRect> {
    // SAFETY: MonitorFromPoint with DEFAULTTOPRIMARY always returns a monitor.
    let m = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    monitor_rect(m)
}

/// The whole area of the screen a window is on.
pub fn monitor_of_window(id: WindowId) -> Option<PxRect> {
    // SAFETY: DEFAULTTONEAREST returns a monitor even for off-screen windows.
    let m = unsafe { MonitorFromWindow(hwnd(id), MONITOR_DEFAULTTONEAREST) };
    monitor_rect(m)
}

/// The dock's registration as an "appbar": the shell keeps other windows out of its space.
/// Dropping it gives the space back.
#[derive(Debug)]
pub struct AppBar {
    window: WindowId,
}

impl AppBar {
    pub fn register(window: WindowId) -> Option<AppBar> {
        let mut data = appbar_data(Some(hwnd(window)));
        data.uCallbackMessage = APPBAR_CALLBACK;
        // SAFETY: `data` is a sized APPBARDATA naming our own window.
        let ok = unsafe { SHAppBarMessage(ABM_NEW, &mut data) };
        (ok != 0).then_some(AppBar { window })
    }

    /// Reserves a strip `thickness` pixels tall on `edge` of `monitor` and returns the strip
    /// the shell granted.
    pub fn reserve(&self, edge: Edge, monitor: PxRect, thickness: i32) -> PxRect {
        let thickness = thickness.clamp(1, monitor.height().max(1));
        let mut data = appbar_data(Some(hwnd(self.window)));
        data.uEdge = match edge {
            Edge::Top => ABE_TOP,
            Edge::Bottom => ABE_BOTTOM,
        };
        data.rc = RECT {
            left: monitor.left,
            top: monitor.top,
            right: monitor.right,
            bottom: monitor.bottom,
        };
        match edge {
            Edge::Top => data.rc.bottom = monitor.top.saturating_add(thickness),
            Edge::Bottom => data.rc.top = monitor.bottom.saturating_sub(thickness),
        }
        // SAFETY: `data` stays valid across both calls; the shell adjusts `rc` in place.
        unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut data) };
        match edge {
            Edge::Top => data.rc.bottom = data.rc.top.saturating_add(thickness),
            Edge::Bottom => data.rc.top = data.rc.bottom.saturating_sub(thickness),
        }
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_SETPOS, &mut data) };
        rect_of(data.rc)
    }
}

impl Drop for AppBar {
    fn drop(&mut self) {
        let mut data = appbar_data(Some(hwnd(self.window)));
        // SAFETY: removes our own registration; harmless if it is already gone.
        unsafe { SHAppBarMessage(ABM_REMOVE, &mut data) };
    }
}

/// The extended-style bits the dock must always have, and the ones it must never have.
const DOCK_EX_ON: u32 = WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0 | WS_EX_TOPMOST.0;
const DOCK_EX_OFF: u32 = WS_EX_APPWINDOW.0;
/// The dock is a plain pop-up. Caption-related styles would make Windows draw a title-bar tint and
/// a close button over the see-through part of the window as soon as it takes the pointer.
const DOCK_STYLE_ON: u32 = WS_POPUP.0;
const DOCK_STYLE_OFF: u32 =
    WS_CAPTION.0 | WS_SYSMENU.0 | WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0;
const DOCK_SUBCLASS_ID: usize = 0xBA2C;

/// Watches the dock window's messages. The window library rewrites the extended style on some
/// events, which would let a click on the dock take focus from the user's app; this keeps the
/// dock's bits in place and refuses activation by mouse outright.
unsafe extern "system" fn dock_subclass(
    h: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    if msg == WM_NCDESTROY {
        // SAFETY: removes this subclass from the window that is being destroyed, as required.
        let _ = unsafe { RemoveWindowSubclass(h, Some(dock_subclass), DOCK_SUBCLASS_ID) };
    }
    if msg == WM_STYLECHANGING && lparam.0 != 0 {
        let which = wparam.0 as i32;
        // SAFETY: for WM_STYLECHANGING, lparam points at the STYLESTRUCT the system passes,
        // which we may edit before the change is applied.
        let change = unsafe { &mut *(lparam.0 as *mut STYLESTRUCT) };
        if which == GWL_EXSTYLE.0 {
            change.styleNew = (change.styleNew | DOCK_EX_ON) & !DOCK_EX_OFF;
        } else if which == GWL_STYLE.0 {
            change.styleNew = (change.styleNew | DOCK_STYLE_ON) & !DOCK_STYLE_OFF;
        }
    }
    // SAFETY: hands every message on to the next handler in the chain, unchanged.
    unsafe { DefSubclassProc(h, msg, wparam, lparam) }
}

/// Makes the dock window stay on top, stay out of Alt+Tab and the taskbar, and never take focus
/// from the app the user is working in.
pub fn style_dock_window(id: WindowId) {
    let h = hwnd(id);
    // SAFETY: subclassing our own window with a procedure that only edits style changes and
    // answers WM_MOUSEACTIVATE; it removes itself on WM_NCDESTROY.
    let installed = unsafe { SetWindowSubclass(h, Some(dock_subclass), DOCK_SUBCLASS_ID, 0) };
    if !installed.as_bool() {
        log::warn!("could not watch the dock window; clicks on it may take focus");
    }
    // SAFETY: reading and writing the styles of our own window, then asking Windows to
    // recompute its frame, which the style change removes.
    unsafe {
        let style = GetWindowLongPtrW(h, GWL_STYLE) as u32;
        let wanted_style = (style | DOCK_STYLE_ON) & !DOCK_STYLE_OFF;
        if wanted_style != style {
            SetWindowLongPtrW(h, GWL_STYLE, wanted_style as isize);
        }
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
        let wanted = (ex | DOCK_EX_ON) & !DOCK_EX_OFF;
        if wanted != ex {
            SetWindowLongPtrW(h, GWL_EXSTYLE, wanted as isize);
        }
        let _ = SetWindowPos(
            h,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Moves and sizes a window, keeping it above other windows and without focusing it.
pub fn place_window(id: WindowId, r: PxRect) {
    // SAFETY: positions our own window.
    let _ = unsafe {
        SetWindowPos(
            hwnd(id),
            Some(HWND_TOPMOST),
            r.left,
            r.top,
            r.width(),
            r.height(),
            SWP_NOACTIVATE,
        )
    };
}

pub fn set_window_shown(id: WindowId, shown: bool) {
    // SAFETY: changes the show state of our own window.
    let _ = unsafe { ShowWindow(hwnd(id), if shown { SW_SHOWNOACTIVATE } else { SW_HIDE }) };
}

/// Limits where a window exists, for drawing and for the mouse, to `area` (in coordinates
/// relative to the window). `None` gives the window back its whole rectangle.
/// Outside the area, clicks reach whatever is underneath.
pub fn set_window_region(id: WindowId, area: Option<PxRect>) {
    let h = hwnd(id);
    match area {
        Some(r) => {
            // SAFETY: the region is handed to the system, which owns it from then on.
            unsafe {
                let rgn = CreateRectRgn(r.left, r.top, r.right, r.bottom);
                if !rgn.is_invalid() {
                    SetWindowRgn(h, Some(rgn), true);
                }
            }
        }
        None => {
            // SAFETY: removing the region from our own window.
            unsafe { SetWindowRgn(h, None, true) };
        }
    }
}

/// The window's display scale: 1.0 at 96 DPI, 1.5 at 144 DPI, and so on.
pub fn dpi_scale(id: WindowId) -> f32 {
    // SAFETY: read-only query; returns 0 for an invalid window.
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd(id)) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    // SAFETY: valid out pointer.
    unsafe { GetCursorPos(&mut p) }.ok()?;
    Some((p.x, p.y))
}

fn primary_tray() -> Option<HWND> {
    // SAFETY: looks a window up by class name.
    unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) }
        .ok()
        .filter(|h| !h.is_invalid())
}

fn all_trays() -> Vec<HWND> {
    let mut trays: Vec<HWND> = primary_tray().into_iter().collect();
    let mut after: Option<HWND> = None;
    for _ in 0..16 {
        // SAFETY: walks top-level windows of the secondary-taskbar class, one after another.
        let next =
            unsafe { FindWindowExW(None, after, w!("Shell_SecondaryTrayWnd"), PCWSTR::null()) };
        match next {
            Ok(h) if !h.is_invalid() => {
                trays.push(h);
                after = Some(h);
            }
            _ => break,
        }
    }
    trays
}

/// Whether the system taskbar is set to auto-hide, or `None` if the shell isn't running.
pub fn taskbar_autohide() -> Option<bool> {
    primary_tray()?;
    let mut data = appbar_data(None);
    // SAFETY: a sized APPBARDATA; ABM_GETSTATE only reads.
    let state = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) } as u32;
    Some(state & ABS_AUTOHIDE != 0)
}

/// Switches the system taskbar's auto-hide setting.
pub fn set_taskbar_autohide(on: bool) {
    let Some(tray) = primary_tray() else {
        return;
    };
    let mut data = appbar_data(Some(tray));
    data.lParam = LPARAM(if on { ABS_AUTOHIDE } else { ABS_ALWAYSONTOP } as isize);
    // SAFETY: a sized APPBARDATA naming the taskbar.
    unsafe { SHAppBarMessage(ABM_SETSTATE, &mut data) };
}

/// Hides the main screen's taskbar, where the dock stands in for it, or shows the taskbar on
/// every screen again. Other screens keep their taskbar while the dock runs: the dock covers only
/// the main screen.
pub fn set_taskbar_shown(shown: bool) {
    let trays = if shown {
        all_trays()
    } else {
        primary_tray().into_iter().collect()
    };
    for tray in trays {
        // SAFETY: changes only the show state of the shell's taskbar windows.
        let _ = unsafe { ShowWindow(tray, if shown { SW_SHOW } else { SW_HIDE }) };
    }
}

pub fn taskbar_shown() -> bool {
    // SAFETY: read-only query.
    primary_tray().is_some_and(|h| unsafe { IsWindowVisible(h) }.as_bool())
}
