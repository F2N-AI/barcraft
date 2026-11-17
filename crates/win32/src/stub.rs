//! Inert stand-ins so the workspace builds on systems other than Windows.

use std::path::{Path, PathBuf};

use crate::{
    Edge, IconRgba, MemoryStatus, PowerStatus, PxRect, Shortcut, TaskbarPin, VolumeStatus,
    WindowId, WindowInfo,
};

#[derive(Debug)]
pub struct AppBar;

impl AppBar {
    pub fn register(_window: WindowId) -> Option<AppBar> {
        None
    }

    pub fn reserve(&self, _edge: Edge, monitor: PxRect, _thickness: i32) -> PxRect {
        monitor
    }
}

#[derive(Debug)]
pub struct SingleInstance;

impl SingleInstance {
    pub fn acquire(_name: &str) -> Option<SingleInstance> {
        Some(SingleInstance)
    }
}

pub fn set_muted(_muted: bool) {}
pub fn set_volume(_level: f32) {}
pub fn volume() -> Option<VolumeStatus> {
    None
}
pub fn cursor_pos() -> Option<(i32, i32)> {
    None
}
pub fn dpi_scale(_id: WindowId) -> f32 {
    1.0
}
pub fn monitor_of_window(_id: WindowId) -> Option<PxRect> {
    None
}
pub fn place_window(_id: WindowId, _r: PxRect) {}
pub fn primary_monitor() -> Option<PxRect> {
    None
}
pub fn set_taskbar_autohide(_on: bool) {}
pub fn set_taskbar_shown(_shown: bool) {}
pub fn set_window_region(_id: WindowId, _area: Option<PxRect>) {}
pub fn set_window_shown(_id: WindowId, _shown: bool) {}
pub fn style_dock_window(_id: WindowId) {}
pub fn taskbar_autohide() -> Option<bool> {
    None
}
pub fn taskbar_shown() -> bool {
    true
}
pub fn icon_for_file(_path: &Path, _size: u32) -> Option<IconRgba> {
    None
}
pub fn icon_for_window(_id: WindowId) -> Option<IconRgba> {
    None
}
pub fn launch_at_startup(_name: &str) -> bool {
    false
}
pub fn set_launch_at_startup(_name: &str, _command: Option<&str>) -> bool {
    false
}
pub fn wait_for_exit(_pid: u32) -> bool {
    false
}
pub fn launch(_path: &Path) -> bool {
    false
}
pub fn open_start_menu() {}
pub fn open_url(_url: &str) -> bool {
    false
}
pub fn resolve_shortcut(_path: &Path) -> Option<Shortcut> {
    None
}
pub fn taskbar_pins() -> Vec<TaskbarPin> {
    Vec::new()
}
pub fn toggle_desktop() {}
pub fn cpu_times() -> Option<(u64, u64, u64)> {
    None
}
pub fn memory_status() -> Option<MemoryStatus> {
    None
}
pub fn power_status() -> Option<PowerStatus> {
    None
}
pub fn activate_window(_id: WindowId) {}
pub fn close_window(_id: WindowId) {}
pub fn end_process(_pid: u32) -> bool {
    false
}
pub fn foreground_is_fullscreen(_own: WindowId, _monitor: PxRect) -> bool {
    false
}
pub fn foreground_window() -> Option<WindowId> {
    None
}
pub fn is_window(_id: WindowId) -> bool {
    false
}
pub fn list_app_windows(_exclude_pid: u32) -> Vec<WindowInfo> {
    Vec::new()
}
pub fn minimize_window(_id: WindowId) {}
pub fn process_exe(_pid: u32) -> Option<PathBuf> {
    None
}
