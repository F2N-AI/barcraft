//! BarCraft: an open-source dock with live widgets that stands in for the Windows taskbar.

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
// No console window behind the dock in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod draw;
mod fonts;
mod poller;
mod settings_kit;
mod settings_window;
mod taskbar;
mod widgets;

use std::path::PathBuf;

use barcraft_core::Settings;

pub const APP_NAME: &str = "BarCraft";
const INSTANCE_MUTEX: &str = "Local\\BarCraft.SingleInstance";

/// Where the settings file lives: `%APPDATA%\BarCraft\settings.json`, or in the folder named
/// by `BARCRAFT_DATA_DIR` (for trying things out without touching the real settings).
pub fn settings_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("BARCRAFT_DATA_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir).join("settings.json");
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    base.join(APP_NAME).join("settings.json")
}

/// Where "Time on PC" keeps today's totals: `%APPDATA%\BarCraft\usage.json`.
pub fn usage_path() -> PathBuf {
    settings_path().with_file_name("usage.json")
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    match Settings::load(&path) {
        Ok(s) => s,
        Err(e) => {
            // Keep the unreadable file: the defaults are saved over it later, and it may hold
            // the user's pinned apps and the taskbar state to restore.
            let backup = path.with_file_name("settings.broken.json");
            match std::fs::copy(&path, &backup) {
                Ok(_) => log::warn!("{e}; kept a copy as {}", backup.display()),
                Err(copy_err) => log::warn!("{e}; could not keep a copy: {copy_err}"),
            }
            Settings::default()
        }
    }
}

pub fn save_settings(settings: &Settings) {
    if let Err(e) = settings.save(&settings_path()) {
        log::warn!("{e}");
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--watchdog") => {
            if let Some(pid) = args.get(1).and_then(|p| p.parse::<u32>().ok()) {
                taskbar::watchdog(pid);
            }
            return;
        }
        Some("--restore-taskbar") => {
            let mut settings = load_settings();
            taskbar::restore(&mut settings);
            return;
        }
        Some("--help") | Some("-h") => {
            println!(
                "{APP_NAME} {}\n\n  barcraft                   start the dock\n  barcraft --open-settings [section]  start the dock with its settings open (at a section, e.g. look)\n  barcraft --preview         draw the bar in an ordinary window, without docking it\n  barcraft --preview --hover N  ...as if the pointer rested on app N (from 0)\n  barcraft --restore-taskbar bring the Windows taskbar back and exit",
                env!("CARGO_PKG_VERSION")
            );
            return;
        }
        _ => {}
    }

    let launch = app::Launch {
        open_settings: args.iter().any(|a| a == "--open-settings"),
        // `--open-settings look` opens it at that section.
        settings_section: args
            .iter()
            .position(|a| a == "--open-settings")
            .and_then(|i| args.get(i + 1))
            .filter(|s| !s.starts_with("--"))
            .cloned(),
        preview: args.iter().any(|a| a == "--preview"),
        hover: args
            .iter()
            .position(|a| a == "--hover")
            .and_then(|i| args.get(i + 1))
            .and_then(|n| n.parse().ok()),
    };
    // A preview may run beside the real dock; only the dock itself must be unique.
    let _instance = if launch.preview {
        None
    } else {
        let Some(lock) = barcraft_win32::SingleInstance::acquire(INSTANCE_MUTEX) else {
            log::info!("{APP_NAME} is already running");
            return;
        };
        Some(lock)
    };

    if let Err(e) = app::run(launch) {
        log::error!("{APP_NAME} stopped: {e}");
    }
}
