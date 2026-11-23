//! System readings for the newer widgets: network, disk, Recycle Bin, idle time, microphone,
//! and shortcuts into the shell (notification centre, system tray).

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole};
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::{SHEmptyRecycleBinW, SHQUERYRBINFO, SHQueryRecycleBinW};
use windows::core::{GUID, IUnknown, PCWSTR};

use super::util::{Com, wide};
use crate::{DiskSpace, NetCounters, RecycleBinStatus, VolumeStatus};

/// IANA interface types we leave out of the totals: loopback and tunnels.
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const IF_TYPE_TUNNEL: u32 = 131;
/// The most adapters we add up; a PC has a handful.
const MAX_ADAPTERS: usize = 256;

/// Bytes moved by the network adapters that are up (loopback and tunnels excluded).
pub fn net_counters() -> Option<NetCounters> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    // SAFETY: GetIfTable2 allocates the table and writes its address into `table`.
    let res = unsafe { GetIfTable2(&mut table) };
    if res.is_err() || table.is_null() {
        return None;
    }
    let mut total = NetCounters::default();
    // SAFETY: on success `table` points at a MIB_IF_TABLE2 whose `Table` holds `NumEntries`
    // rows laid out one after another; we read within that count and free it below.
    unsafe {
        let count = ((*table).NumEntries as usize).min(MAX_ADAPTERS);
        let rows = std::ptr::addr_of!((*table).Table)
            .cast::<windows::Win32::NetworkManagement::IpHelper::MIB_IF_ROW2>();
        for i in 0..count {
            let row = &*rows.add(i);
            // Bit 0 of the flags marks a hardware adapter.
            let hardware = row.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
            if !hardware
                || row.OperStatus != IfOperStatusUp
                || row.Type == IF_TYPE_SOFTWARE_LOOPBACK
                || row.Type == IF_TYPE_TUNNEL
            {
                continue;
            }
            total.received = total.received.saturating_add(row.InOctets);
            total.sent = total.sent.saturating_add(row.OutOctets);
        }
        FreeMibTable(table.cast());
    }
    Some(total)
}

/// Free and total space on the drive Windows is installed on.
pub fn system_drive_space() -> Option<DiskSpace> {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
    let root = wide(&format!("{drive}\\"));
    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: `root` is NUL-terminated; both out pointers are valid.
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(root.as_ptr()),
            Some(&mut free),
            Some(&mut total),
            None,
        )
    }
    .ok()?;
    (total > 0).then_some(DiskSpace { free, total })
}

/// What the Recycle Bin holds, on all drives.
pub fn recycle_bin() -> Option<RecycleBinStatus> {
    let mut info = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: a null root path means every drive; `info` has its size set.
    unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) }.ok()?;
    Some(RecycleBinStatus {
        items: u64::try_from(info.i64NumItems).unwrap_or(0),
        bytes: u64::try_from(info.i64Size).unwrap_or(0),
    })
}

/// Empties the Recycle Bin. Windows asks the user to confirm first, with its own dialog.
pub fn empty_recycle_bin() {
    // SAFETY: no owner window, every drive, default flags (which keep the confirmation).
    let _ = unsafe { SHEmptyRecycleBinW(None, PCWSTR::null(), 0) };
}

/// Seconds since the last keyboard or mouse input.
pub fn idle_seconds() -> Option<u32> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` has its size set.
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return None;
    }
    // SAFETY: no arguments. Both counters wrap together, so the difference stays right.
    let now = unsafe { GetTickCount() };
    Some(now.wrapping_sub(info.dwTime) / 1000)
}

fn microphone_endpoint() -> Option<IAudioEndpointVolume> {
    // SAFETY: standard COM creation and activation; interfaces release when dropped.
    unsafe {
        let devices: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None::<&IUnknown>, CLSCTX_ALL).ok()?;
        let mic = devices.GetDefaultAudioEndpoint(eCapture, eConsole).ok()?;
        mic.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok()
    }
}

/// The default microphone's level and mute state, or `None` without a microphone.
pub fn microphone() -> Option<VolumeStatus> {
    let _com = Com::init();
    let ep = microphone_endpoint()?;
    // SAFETY: plain getters on a live interface.
    let level = unsafe { ep.GetMasterVolumeLevelScalar() }.ok()?;
    // SAFETY: as above.
    let muted = unsafe { ep.GetMute() }.ok()?.as_bool();
    Some(VolumeStatus {
        level: level.clamp(0.0, 1.0),
        muted,
    })
}

pub fn set_microphone_muted(muted: bool) {
    let _com = Com::init();
    if let Some(ep) = microphone_endpoint() {
        // SAFETY: a null event-context GUID is allowed.
        let _ = unsafe { ep.SetMute(muted, std::ptr::null::<GUID>()) };
    }
}
