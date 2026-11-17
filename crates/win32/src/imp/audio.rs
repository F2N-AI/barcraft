//! Speaker volume through the Core Audio endpoint interface.

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::{GUID, IUnknown};

use super::util::Com;
use crate::VolumeStatus;

fn endpoint() -> Option<IAudioEndpointVolume> {
    // SAFETY: standard COM creation and activation; interfaces release when dropped.
    unsafe {
        let devices: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None::<&IUnknown>, CLSCTX_ALL).ok()?;
        let speaker = devices.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        speaker
            .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            .ok()
    }
}

/// The default speaker's volume, or `None` when there is no output device.
pub fn volume() -> Option<VolumeStatus> {
    let _com = Com::init();
    let ep = endpoint()?;
    // SAFETY: plain getters on a live interface.
    let level = unsafe { ep.GetMasterVolumeLevelScalar() }.ok()?;
    // SAFETY: as above.
    let muted = unsafe { ep.GetMute() }.ok()?.as_bool();
    Some(VolumeStatus {
        level: level.clamp(0.0, 1.0),
        muted,
    })
}

pub fn set_volume(level: f32) {
    if !level.is_finite() {
        return;
    }
    let _com = Com::init();
    if let Some(ep) = endpoint() {
        // SAFETY: a null event-context GUID is allowed.
        let _ = unsafe {
            ep.SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), std::ptr::null::<GUID>())
        };
    }
}

pub fn set_muted(muted: bool) {
    let _com = Com::init();
    if let Some(ep) = endpoint() {
        // SAFETY: a null event-context GUID is allowed.
        let _ = unsafe { ep.SetMute(muted, std::ptr::null::<GUID>()) };
    }
}
