//! Reading program, shortcut and window icons as RGBA pixels.

use std::ffi::c_void;
use std::path::Path;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GCLP_HICON, GetClassLongPtrW, GetIconInfo, HICON, ICON_BIG, ICONINFO,
    PrivateExtractIconsW, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_GETICON,
};
use windows::core::PCWSTR;

use super::shell::resolve_shortcut;
use super::util::{hwnd, wide_os};
use crate::{IconRgba, WindowId};

/// Icons larger than this are refused; nothing on a dock needs more.
const MAX_SIDE: i32 = 256;

/// The icon Windows shows for a file, at about `size` pixels.
/// Shortcuts use their own icon, or their target's.
pub fn icon_for_file(path: &Path, size: u32) -> Option<IconRgba> {
    let is_link = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("lnk"));
    if is_link && let Some(link) = resolve_shortcut(path) {
        if let Some((icon_file, index)) = &link.icon
            && let Some(icon) = extract(icon_file, *index, size)
        {
            return Some(icon);
        }
        if let Some(target) = &link.target
            && let Some(icon) = extract(target, 0, size)
        {
            return Some(icon);
        }
    }
    extract(path, 0, size).or_else(|| shell_icon(path))
}

/// The icon a window shows in its title bar.
pub fn icon_for_window(id: WindowId) -> Option<IconRgba> {
    let h = hwnd(id);
    let mut result = 0usize;
    // SAFETY: WM_GETICON carries no pointers; the timeout keeps a hung window from blocking us.
    unsafe {
        SendMessageTimeoutW(
            h,
            WM_GETICON,
            WPARAM(ICON_BIG as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            100,
            Some(&mut result),
        )
    };
    if result == 0 {
        // SAFETY: read-only class query.
        result = unsafe { GetClassLongPtrW(h, GCLP_HICON) };
    }
    if result == 0 {
        return None;
    }
    // The window owns this icon: we copy its pixels and must not destroy it.
    hicon_to_rgba(HICON(result as *mut c_void))
}

fn extract(path: &Path, index: i32, size: u32) -> Option<IconRgba> {
    let wide = wide_os(path.as_os_str());
    let mut name = [0u16; 260];
    if wide.len() > name.len() {
        return None;
    }
    name.get_mut(..wide.len())?.copy_from_slice(&wide);
    let side = i32::try_from(size).ok()?.clamp(16, MAX_SIDE);
    let mut icons = [HICON::default()];
    let mut ids = [0u32];
    // SAFETY: `name` is NUL-terminated; `icons` and `ids` hold one entry each, matching the
    // one icon requested.
    let n = unsafe {
        PrivateExtractIconsW(
            &name,
            index,
            side,
            side,
            Some(&mut icons),
            Some(ids.as_mut_ptr()),
            0,
        )
    };
    let icon = icons[0];
    if n == 0 || n == u32::MAX || icon.is_invalid() {
        return None;
    }
    let pixels = hicon_to_rgba(icon);
    // SAFETY: PrivateExtractIconsW created this icon for us; destroyed exactly once.
    let _ = unsafe { DestroyIcon(icon) };
    pixels
}

fn shell_icon(path: &Path) -> Option<IconRgba> {
    let wide = wide_os(path.as_os_str());
    let mut info = SHFILEINFOW::default();
    // SAFETY: `wide` is NUL-terminated and `info` is sized as declared.
    let ok = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if ok == 0 || info.hIcon.is_invalid() {
        return None;
    }
    let pixels = hicon_to_rgba(info.hIcon);
    // SAFETY: SHGetFileInfoW handed us this icon to destroy.
    let _ = unsafe { DestroyIcon(info.hIcon) };
    pixels
}

fn bitmap_size(bitmap: HBITMAP) -> Option<(i32, i32)> {
    let mut bm = BITMAP::default();
    // SAFETY: `bm` is a BITMAP and the size passed matches it.
    let got = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut BITMAP as *mut c_void),
        )
    };
    if got == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
        return None;
    }
    Some((bm.bmWidth, bm.bmHeight))
}

/// Reads a bitmap as 32-bit top-down BGRA.
fn read_bgra(dc: HDC, bitmap: HBITMAP, width: i32, height: i32) -> Option<Vec<u8>> {
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let len = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)?;
    let mut buf = vec![0u8; len];
    // SAFETY: `buf` holds width*height 32-bit pixels, exactly what `info` asks for.
    let lines = unsafe {
        GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(buf.as_mut_ptr() as *mut c_void),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    if lines <= 0 { None } else { Some(buf) }
}

fn hicon_to_rgba(icon: HICON) -> Option<IconRgba> {
    let mut parts = ICONINFO::default();
    // SAFETY: `parts` receives two bitmaps that we delete below.
    unsafe { GetIconInfo(icon, &mut parts) }.ok()?;

    let pixels = (|| {
        if parts.hbmColor.is_invalid() {
            return None; // Monochrome icon; rare enough to fall back to a letter tile.
        }
        let (w, h) = bitmap_size(parts.hbmColor)?;
        if w > MAX_SIDE || h > MAX_SIDE {
            return None;
        }
        // SAFETY: a memory DC compatible with the screen, deleted below.
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            return None;
        }
        let mut bgra = read_bgra(dc, parts.hbmColor, w, h);
        let has_alpha = bgra
            .as_ref()
            .is_some_and(|b| b.as_chunks::<4>().0.iter().any(|[_, _, _, a]| *a != 0));
        if let Some(buf) = bgra.as_mut()
            && !has_alpha
        {
            // Old-style icon: opacity comes from the mask (black = opaque).
            let mask = if parts.hbmMask.is_invalid() {
                None
            } else {
                read_bgra(dc, parts.hbmMask, w, h)
            };
            for (i, [_, _, _, a]) in buf.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let opaque = mask
                    .as_ref()
                    .and_then(|m| m.get(i * 4))
                    .is_none_or(|&v| v == 0);
                *a = if opaque { 255 } else { 0 };
            }
        }
        // SAFETY: created above, deleted once.
        let _ = unsafe { DeleteDC(dc) };
        let mut rgba = bgra?;
        for px in rgba.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        Some(IconRgba {
            width: w as u32,
            height: h as u32,
            rgba,
        })
    })();

    // SAFETY: GetIconInfo gave us these bitmaps to delete.
    unsafe {
        if !parts.hbmColor.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(parts.hbmColor.0));
        }
        if !parts.hbmMask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(parts.hbmMask.0));
        }
    }
    pixels
}
