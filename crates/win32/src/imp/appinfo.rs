//! What an app calls itself: the Start menu's name for packaged (Store) apps, and the names in
//! a program file's version information for the rest.

use std::ffi::c_void;
use std::path::Path;

use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Com::{CoTaskMemFree, IBindCtx};
use windows::Win32::UI::Shell::{IShellItem, SHCreateItemFromParsingName, SIGDN_NORMALDISPLAY};
use windows::core::{Interface, PCWSTR, PWSTR};

use super::util::{Com, string_from_wide, wide, wide_os};
use crate::VersionNames;

/// Version blocks larger than this are refused; real ones are a few kilobytes.
const MAX_VERSION_BLOCK: u32 = 1024 * 1024;

/// The packaged (Store) app a process belongs to, as Windows identifies it (its "AppUserModelID",
/// e.g. `Microsoft.WindowsTerminal_8wekyb3d8bbwe!App`). `None` for ordinary programs.
pub(crate) fn app_user_model_id(process: HANDLE) -> Option<String> {
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` and `len` describe the same writable buffer; `process` is a live handle
    // with query rights, owned by the caller.
    let res =
        unsafe { GetApplicationUserModelId(process, &mut len, Some(PWSTR(buf.as_mut_ptr()))) };
    if res != ERROR_SUCCESS {
        return None;
    }
    let id = string_from_wide(&buf);
    (!id.is_empty()).then_some(id)
}

/// The Start menu's entry for a packaged app, as a shell item of type `T`.
pub(crate) fn apps_folder_item<T: Interface>(app_id: &str) -> Option<T> {
    if app_id.is_empty() || app_id.contains(['\\', '/']) {
        return None;
    }
    let path = wide(&format!("shell:AppsFolder\\{app_id}"));
    // SAFETY: `path` is NUL-terminated and outlives the call; the item is released when dropped.
    unsafe { SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None::<&IBindCtx>) }.ok()
}

/// The name the Start menu shows for a packaged app ("Terminal", "WhatsApp").
pub fn packaged_app_name(app_id: &str) -> Option<String> {
    let _com = Com::init();
    let item: IShellItem = apps_folder_item(app_id)?;
    // SAFETY: asks the item for its display name; the string is ours to free below.
    let name = unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY) }.ok()?;
    // SAFETY: `name` is a NUL-terminated string the shell just allocated.
    let text = unsafe { name.to_string() }.ok();
    // SAFETY: the shell allocated `name` with the COM allocator; freed exactly once.
    unsafe { CoTaskMemFree(Some(name.0 as *const c_void)) };
    text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

/// The description and product name in a program file's version information.
pub fn version_names(path: &Path) -> VersionNames {
    let file = wide_os(path.as_os_str());
    // SAFETY: `file` is NUL-terminated and outlives the call.
    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(file.as_ptr()), None) };
    if size == 0 || size > MAX_VERSION_BLOCK {
        return VersionNames::default();
    }
    let mut block = vec![0u8; size as usize];
    // SAFETY: `block` is `size` writable bytes, as the call is told.
    let read = unsafe {
        GetFileVersionInfoW(
            PCWSTR(file.as_ptr()),
            None,
            size,
            block.as_mut_ptr() as *mut c_void,
        )
    };
    if read.is_err() {
        return VersionNames::default();
    }
    // The languages the strings are stored in, then the usual ones in case none is listed.
    let mut codes: Vec<String> = query(&block, "\\VarFileInfo\\Translation")
        .map(|bytes| {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|[a, b, c, d]| {
                    let lang = u16::from_le_bytes([*a, *b]);
                    let page = u16::from_le_bytes([*c, *d]);
                    format!("{lang:04x}{page:04x}")
                })
                .collect()
        })
        .unwrap_or_default();
    for fallback in ["040904b0", "040904e4", "000004b0"] {
        if !codes.iter().any(|c| c == fallback) {
            codes.push(fallback.to_owned());
        }
    }
    let field = |name: &str| {
        codes.iter().find_map(|code| {
            let bytes = query(&block, &format!("\\StringFileInfo\\{code}\\{name}"))?;
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .collect();
            let text = string_from_wide(&units);
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_owned())
        })
    };
    VersionNames {
        description: field("FileDescription"),
        product: field("ProductName"),
    }
}

/// The bytes of one value in a version block. Strings come back as UTF-16 with their NUL.
fn query<'a>(block: &'a [u8], sub_block: &str) -> Option<&'a [u8]> {
    let key = wide(sub_block);
    let mut ptr: *mut c_void = std::ptr::null_mut();
    let mut len = 0u32;
    // SAFETY: `block` is a version block filled by GetFileVersionInfoW and outlives the call;
    // the call only writes the two out-parameters.
    let ok = unsafe {
        VerQueryValueW(
            block.as_ptr() as *const c_void,
            PCWSTR(key.as_ptr()),
            &mut ptr,
            &mut len,
        )
    };
    if !ok.as_bool() || ptr.is_null() || len == 0 {
        return None;
    }
    // The value points into `block`; read it through the slice, refusing anything outside it.
    let start = (ptr as usize).checked_sub(block.as_ptr() as usize)?;
    let bytes = if sub_block.starts_with("\\StringFileInfo") {
        (len as usize).checked_mul(2)?
    } else {
        len as usize
    };
    block.get(start..start.checked_add(bytes)?)
}
