//! Win32 implementation. Each submodule keeps its `unsafe` blocks small and explained.

mod audio;
mod dock;
mod icons;
mod process;
mod shell;
mod sys;
mod util;
mod wnd;

pub use audio::{set_muted, set_volume, volume};
pub use dock::{
    AppBar, cursor_pos, dpi_scale, monitor_of_window, place_window, primary_monitor,
    set_taskbar_autohide, set_taskbar_shown, set_window_region, set_window_shown,
    style_dock_window, taskbar_autohide, taskbar_shown,
};
pub use icons::{icon_for_file, icon_for_window};
pub use process::{SingleInstance, launch_at_startup, set_launch_at_startup, wait_for_exit};
pub use shell::{
    launch, open_start_menu, open_url, resolve_shortcut, taskbar_pins, toggle_desktop,
};
pub use sys::{cpu_times, memory_status, power_status};
pub use wnd::{
    activate_window, close_window, end_process, foreground_is_fullscreen, foreground_window,
    is_window, list_app_windows, minimize_window, process_exe,
};
