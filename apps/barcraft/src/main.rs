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
mod poller;
mod settings_window;
mod taskbar;
mod widgets;

use std::path::PathBuf;

use barcraft_core::Settings;

pub const APP_NAME: &str = "BarCraft";
const INSTANCE_MUTEX: &str = "Local\\BarCraft.SingleInstance";

/// Where the settings file lives: `%APPDATA%\BarCraft\settings.json`.
pub fn settings_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    base.join(APP_NAME).join("settings.json")
}

pub fn load_settings() -> Settings {
    match Settings::load(&settings_path()) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("{e}; starting from the default settings");
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
                "{APP_NAME} {}\n\n  barcraft                   start the dock\n  barcraft --restore-taskbar bring the Windows taskbar back and exit",
                env!("CARGO_PKG_VERSION")
            );
            return;
        }
        _ => {}
    }

    let Some(_instance) = barcraft_win32::SingleInstance::acquire(INSTANCE_MUTEX) else {
        log::info!("{APP_NAME} is already running");
        return;
    };

    if let Err(e) = app::run() {
        log::error!("{APP_NAME} stopped: {e}");
    }
}
