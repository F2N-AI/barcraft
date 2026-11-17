//! The one crate in BarCraft that calls Win32.
//!
//! Every `unsafe` block in the project lives in this crate, each with a `// SAFETY:` note,
//! behind functions that are safe to call and that never panic. Calls that can fail return
//! `Option` or `Result`; calls whose failure only means "nothing happened" return nothing.
//!
//! On other systems the crate builds with inert stand-ins, so the rest of the workspace
//! (and its tests) still compile there.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::path::PathBuf;

/// A top-level window, as the handle's numeric value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub isize);

/// One window that would show on a taskbar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub pid: u32,
    pub exe: Option<PathBuf>,
    pub minimized: bool,
}

/// A rectangle in physical screen pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PxRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PxRect {
    pub fn width(&self) -> i32 {
        self.right.saturating_sub(self.left)
    }

    pub fn height(&self) -> i32 {
        self.bottom.saturating_sub(self.top)
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// An icon decoded to straight-alpha RGBA bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconRgba {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A shortcut (`.lnk`) file, resolved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shortcut {
    pub target: Option<PathBuf>,
    pub icon: Option<(PathBuf, i32)>,
}

/// A shortcut the user pinned to the system taskbar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskbarPin {
    pub link: PathBuf,
    pub name: String,
    pub target: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryStatus {
    pub load_percent: u32,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerStatus {
    pub has_battery: bool,
    pub charging: bool,
    pub on_mains: bool,
    pub percent: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VolumeStatus {
    /// From 0 to 1.
    pub level: f32,
    pub muted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
}

#[cfg(windows)]
mod imp;
#[cfg(windows)]
pub use imp::*;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::*;
