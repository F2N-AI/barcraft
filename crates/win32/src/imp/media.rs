//! What's playing, through the system media session that music players and browsers report to.
//! This is a WinRT API; it is called from the poller thread inside a multithreaded COM
//! apartment, where blocking on its results is allowed.

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

use crate::{MediaAction, MediaStatus};

/// A multithreaded COM apartment for this thread, released when dropped if this call started it.
struct Mta {
    started: bool,
}

impl Mta {
    fn init() -> Mta {
        // SAFETY: balanced by CoUninitialize in Drop when it succeeded.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Mta {
            started: hr.is_ok(),
        }
    }
}

impl Drop for Mta {
    fn drop(&mut self) {
        if self.started {
            // SAFETY: matches the successful CoInitializeEx on this thread.
            unsafe { CoUninitialize() };
        }
    }
}

fn current_session() -> Option<GlobalSystemMediaTransportControlsSession> {
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .ok()?
        .join()
        .ok()?;
    manager.GetCurrentSession().ok()
}

/// The media session Windows considers current, if any app is reporting one.
pub fn media_status() -> Option<MediaStatus> {
    let _apartment = Mta::init();
    let session = current_session()?;
    let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let title = props
        .Title()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let artist = props
        .Artist()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let playing = session
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .is_ok_and(|s| s == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing);
    if title.is_empty() && artist.is_empty() {
        return None;
    }
    Some(MediaStatus {
        title,
        artist,
        playing,
    })
}

/// Sends play/pause, next or previous to the current media session.
pub fn media_action(action: MediaAction) {
    let _apartment = Mta::init();
    let Some(session) = current_session() else {
        return;
    };
    let request = match action {
        MediaAction::PlayPause => session.TryTogglePlayPauseAsync(),
        MediaAction::Next => session.TrySkipNextAsync(),
        MediaAction::Previous => session.TrySkipPreviousAsync(),
    };
    if let Ok(op) = request {
        let _ = op.join();
    }
}
