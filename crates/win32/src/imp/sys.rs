//! Processor, memory and battery readings.

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

use crate::{MemoryStatus, PowerStatus};

fn ticks(t: FILETIME) -> u64 {
    (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
}

/// Cumulative (idle, kernel, user) processor time since boot, in 100 ns ticks.
/// Kernel time includes idle time.
pub fn cpu_times() -> Option<(u64, u64, u64)> {
    let (mut idle, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: three valid out pointers.
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    Some((ticks(idle), ticks(kernel), ticks(user)))
}

pub fn memory_status() -> Option<MemoryStatus> {
    let mut m = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `m` is a MEMORYSTATUSEX with its length field set, as the call requires.
    unsafe { GlobalMemoryStatusEx(&mut m) }.ok()?;
    Some(MemoryStatus {
        load_percent: m.dwMemoryLoad.min(100),
        total_bytes: m.ullTotalPhys,
        available_bytes: m.ullAvailPhys,
    })
}

pub fn power_status() -> Option<PowerStatus> {
    let mut p = SYSTEM_POWER_STATUS::default();
    // SAFETY: valid out pointer.
    unsafe { GetSystemPowerStatus(&mut p) }.ok()?;
    const NO_BATTERY: u8 = 128;
    const CHARGING: u8 = 8;
    const UNKNOWN: u8 = 255;
    let has_battery = p.BatteryFlag != UNKNOWN && p.BatteryFlag & NO_BATTERY == 0;
    Some(PowerStatus {
        has_battery,
        charging: has_battery && p.BatteryFlag & CHARGING != 0,
        on_mains: p.ACLineStatus == 1,
        percent: (p.BatteryLifePercent <= 100).then_some(p.BatteryLifePercent),
    })
}
