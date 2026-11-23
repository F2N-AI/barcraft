//! A window that sits right under the dock's bar and asks Windows to blur what's behind it, so
//! the bar reads as frosted glass. The dock keeps drawing the bar itself on top.
//!
//! The dock's own window can't carry the blur: it also holds the menus and the calendar, so a
//! blur on it would frost the whole area above the bar whenever a menu opens.

use std::ffi::c_void;
use std::sync::OnceLock;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DROPSHADOW, CreateWindowExW, DefWindowProcW, DestroyWindow, IsWindowVisible,
    RegisterClassExW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetWindowPos, ShowWindow,
    WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{BOOL, PCWSTR, s, w};

use super::util::{hwnd, id_of};
use crate::{PxRect, WindowId};

const CLASS_NAME: PCWSTR = w!("BarCraftBackdrop");

/// `WINDOWCOMPOSITIONATTRIB::WCA_ACCENT_POLICY`.
const WCA_ACCENT_POLICY: u32 = 19;
/// `ACCENT_STATE::ACCENT_ENABLE_ACRYLICBLURBEHIND`: blur plus a faint grain, as the shell uses.
const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    /// Tint as 0xAABBGGRR. The dock paints its own tint, so this one is almost clear.
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct CompositionData {
    attribute: u32,
    data: *mut c_void,
    size: usize,
}

type SetWindowCompositionAttribute = unsafe extern "system" fn(HWND, *mut CompositionData) -> BOOL;

/// The shell's blur-behind switch. It is not in the Windows SDK but has been in user32 since
/// Windows 10 and is what every tool that frosts the taskbar uses; without it the bar keeps a
/// plain tint.
fn composition_api() -> Option<SetWindowCompositionAttribute> {
    static API: OnceLock<Option<usize>> = OnceLock::new();
    let addr = *API.get_or_init(|| {
        // SAFETY: user32 is loaded in every GUI process; the name is a NUL-terminated literal.
        let module = unsafe { GetModuleHandleW(w!("user32.dll")) }.ok()?;
        // SAFETY: as above; a missing export gives None.
        let proc = unsafe { GetProcAddress(module, s!("SetWindowCompositionAttribute")) }?;
        Some(proc as usize)
    });
    // SAFETY: `addr` is the address of SetWindowCompositionAttribute in user32, whose signature
    // is (HWND, WINDOWCOMPOSITIONATTRIBDATA*) -> BOOL, matching the type we give it.
    addr.map(|a| unsafe { std::mem::transmute::<usize, SetWindowCompositionAttribute>(a) })
}

unsafe extern "system" fn backdrop_proc(
    h: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: the default handling for every message; the window has no state of its own.
    unsafe { DefWindowProcW(h, msg, wparam, lparam) }
}

fn register_class() -> Option<HINSTANCE> {
    static REGISTERED: OnceLock<Option<isize>> = OnceLock::new();
    let instance = *REGISTERED.get_or_init(|| {
        // SAFETY: the module handle of this executable.
        let module = unsafe { GetModuleHandleW(PCWSTR::null()) }.ok()?;
        let instance = HINSTANCE(module.0);
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(backdrop_proc),
            // A soft shadow around the glass, drawn by Windows and rounded with the corners.
            style: CS_DROPSHADOW,
            hInstance: instance,
            // Painted black, which the compositor treats as see-through here, so only the blur shows.
            // SAFETY: stock objects are owned by the system and never freed.
            hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `class` is a fully initialised WNDCLASSEXW whose strings are 'static.
        let atom = unsafe { RegisterClassExW(&class) };
        (atom != 0).then_some(instance.0 as isize)
    });
    instance.map(|i| HINSTANCE(i as *mut c_void))
}

/// The blurred backdrop under the bar. Dropping it destroys the window.
#[derive(Debug)]
pub struct Backdrop {
    window: WindowId,
    placed: Option<PxRect>,
}

impl Backdrop {
    /// The radius Windows 11 gives rounded window corners, in points. The blur ignores window
    /// regions, so this rounding is the only way to round it, and the bar matches it.
    pub const CORNER_RADIUS_POINTS: f32 = 8.0;

    /// `None` when the window can't be made or Windows offers no blur.
    pub fn create() -> Option<Backdrop> {
        let api = composition_api()?;
        let instance = register_class()?;
        // SAFETY: a plain pop-up of the class registered above; destroyed in Drop.
        let h = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                CLASS_NAME,
                w!("BarCraft backdrop"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(instance),
                None,
            )
        }
        .ok()?;
        let mut accent = AccentPolicy {
            accent_state: ACCENT_ENABLE_ACRYLICBLURBEHIND,
            accent_flags: 0,
            gradient_color: 0x0100_0000,
            animation_id: 0,
        };
        let mut data = CompositionData {
            attribute: WCA_ACCENT_POLICY,
            data: (&mut accent as *mut AccentPolicy).cast(),
            size: std::mem::size_of::<AccentPolicy>(),
        };
        // SAFETY: `data` points at a live AccentPolicy of the declared size for the call.
        let ok = unsafe { api(h, &mut data) }.as_bool();
        if !ok {
            // SAFETY: destroying the window we just created.
            let _ = unsafe { DestroyWindow(h) };
            return None;
        }
        // Ask Windows 11 to round the corners; it clips the blur too. Earlier versions ignore it
        // and keep square corners.
        let round = DWMWCP_ROUND;
        // SAFETY: the attribute takes a DWM_WINDOW_CORNER_PREFERENCE, passed by pointer with its size.
        let _ = unsafe {
            DwmSetWindowAttribute(
                h,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&round as *const windows::Win32::Graphics::Dwm::DWM_WINDOW_CORNER_PREFERENCE)
                    .cast(),
                std::mem::size_of_val(&round) as u32,
            )
        };
        Some(Backdrop {
            window: id_of(h),
            placed: None,
        })
    }

    pub fn window(&self) -> WindowId {
        self.window
    }

    /// Puts the backdrop exactly under `rect` (screen pixels), directly below the dock window in
    /// the stacking order. Its corners are rounded by Windows, with a radius of
    /// [`Self::CORNER_RADIUS_POINTS`].
    pub fn place(&mut self, dock: WindowId, rect: PxRect) {
        let h = hwnd(self.window);
        // SAFETY: positions our own window; inserting after the (topmost) dock keeps it topmost
        // and right under the dock.
        let _ = unsafe {
            SetWindowPos(
                h,
                Some(hwnd(dock)),
                rect.left,
                rect.top,
                rect.width().max(1),
                rect.height().max(1),
                SWP_NOACTIVATE,
            )
        };
        self.placed = Some(rect);
    }

    /// Shows or hides the backdrop. Reads the real state each time, because the poller also
    /// hides it (with the dock) for full-screen apps.
    pub fn set_shown(&mut self, shown: bool) {
        let h = hwnd(self.window);
        // SAFETY: read-only query on our own window.
        let visible = unsafe { IsWindowVisible(h) }.as_bool();
        if visible == shown {
            return;
        }
        // SAFETY: changes the show state of our own window without activating it.
        let _ = unsafe { ShowWindow(h, if shown { SW_SHOWNOACTIVATE } else { SW_HIDE }) };
    }
}

impl Drop for Backdrop {
    fn drop(&mut self) {
        // SAFETY: destroys the window this value created, once.
        let _ = unsafe { DestroyWindow(hwnd(self.window)) };
    }
}
