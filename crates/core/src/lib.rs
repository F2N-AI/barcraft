//! Platform-independent model for BarCraft.
//!
//! Nothing here talks to the operating system: settings, themes, the widget catalogue,
//! how open windows and pinned apps merge into dock items, and the small calculations
//! (CPU load, calendar grid, focus timer, weather codes) the widgets show.

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod apps;
pub mod calendar;
pub mod clock;
pub mod cpu;
pub mod focus;
pub mod settings;
pub mod theme;
pub mod usage;
pub mod weather;
pub mod widgets;

pub use apps::{DockItem, PinnedApp, WindowRef, build_dock_items, normalize_path};
pub use settings::{
    DockEdge, DockLayout, DockSize, IndicatorStyle, Settings, TaskbarMode, TempUnit,
};
pub use theme::{Rgba, Theme, ThemeId};
pub use widgets::WidgetKind;
