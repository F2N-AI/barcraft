//! Win32 implementation. Each submodule keeps its `unsafe` blocks small and explained.

mod appinfo;
mod audio;
mod backdrop;
mod dock;
mod extra;
mod icons;
mod media;
mod process;
mod shell;
mod sys;
mod util;
mod wnd;

pub use appinfo::{packaged_app_name, version_names};
pub use audio::{set_muted, set_volume, volume};
pub use backdrop::Backdrop;
pub use dock::{
    AppBar, cursor_pos, dpi_scale, monitor_of_window, place_window, primary_monitor,
    set_taskbar_autohide, set_taskbar_shown, set_window_region, set_window_shown,
    style_dock_window, taskbar_autohide, taskbar_shown,
};
pub use extra::{
    empty_recycle_bin, idle_seconds, microphone, net_counters, recycle_bin, set_microphone_muted,
    system_drive_space,
};
pub use icons::{icon_for_app, icon_for_file, icon_for_window};
pub use media::{media_action, media_status};
pub use process::{
    SingleInstance, launch_at_startup, set_launch_at_startup, system_uses_light_theme,
    wait_for_exit,
};
pub use shell::{
    focus_system_tray, launch, open_notification_center, open_recycle_bin, open_start_menu,
    open_url, resolve_shortcut, taskbar_pins, toggle_desktop,
};
pub use sys::{cpu_times, memory_status, power_status};
pub use wnd::{
    activate_window, close_window, end_process, foreground_is_fullscreen, foreground_is_shell_ui,
    foreground_window, is_window, list_app_windows, minimize_window, process_exe,
};
