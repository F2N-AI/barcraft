//! Hiding the Windows taskbar while the dock runs, and always putting it back.
//!
//! Three layers make sure the taskbar returns:
//! 1. a clean exit restores it;
//! 2. a watchdog process waits for the dock to end, however it ends, and restores it;
//! 3. the original state is written to the settings file before anything changes, so the next
//!    start (or `barcraft --restore-taskbar`) can restore it even after a power cut.

use barcraft_core::settings::SavedTaskbar;
use barcraft_core::{Settings, TaskbarMode};

use crate::save_settings;

/// Applies the user's taskbar choice.
pub fn apply(settings: &mut Settings) {
    match settings.taskbar_mode {
        TaskbarMode::Untouched => restore(settings),
        mode @ (TaskbarMode::AutoHide | TaskbarMode::Hidden) => {
            if settings.saved_taskbar.is_none() {
                // Record what we found before changing it. If the shell isn't running
                // there's nothing to change.
                let Some(autohide) = barcraft_win32::taskbar_autohide() else {
                    return;
                };
                settings.saved_taskbar = Some(SavedTaskbar { autohide });
                save_settings(settings);
            }
            // Auto-hide frees the screen space the taskbar reserves; hiding takes the
            // taskbar off screen so it can't slide over the dock.
            barcraft_win32::set_taskbar_autohide(true);
            barcraft_win32::set_taskbar_shown(mode != TaskbarMode::Hidden);
        }
    }
}

/// Re-hides the taskbar if the shell brought it back (it does after some system events).
pub fn keep_applied(mode: TaskbarMode) {
    if mode == TaskbarMode::Hidden && barcraft_win32::taskbar_shown() {
        barcraft_win32::set_taskbar_shown(false);
    }
}

/// Puts the taskbar back the way it was before BarCraft changed it.
pub fn restore(settings: &mut Settings) {
    barcraft_win32::set_taskbar_shown(true);
    if let Some(saved) = settings.saved_taskbar.take() {
        barcraft_win32::set_taskbar_autohide(saved.autohide);
        save_settings(settings);
    }
}

/// Starts a copy of this program that waits for us to end and then restores the taskbar.
pub fn spawn_watchdog() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--watchdog").arg(std::process::id().to_string());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(DETACHED_PROCESS);
    }
    if let Err(e) = cmd.spawn() {
        log::warn!("could not start the taskbar watchdog: {e}");
    }
}

/// The watchdog's whole life: wait, then restore if the dock didn't.
pub fn watchdog(pid: u32) {
    if !barcraft_win32::wait_for_exit(pid) {
        return;
    }
    let mut settings = crate::load_settings();
    if settings.saved_taskbar.is_some() {
        log::info!("the dock ended without restoring the taskbar; restoring it now");
        restore(&mut settings);
    } else {
        barcraft_win32::set_taskbar_shown(true);
    }
}
