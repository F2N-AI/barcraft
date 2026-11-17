//! User settings, stored as JSON next to the user's other app data.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::apps::PinnedApp;
use crate::theme::ThemeId;
use crate::widgets::WidgetKind;

/// Which screen edge the dock sits on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockEdge {
    #[default]
    Bottom,
    Top,
}

/// Overall dock size: icon size, bar height and text size follow from it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl DockSize {
    pub const ALL: [DockSize; 3] = [DockSize::Small, DockSize::Medium, DockSize::Large];

    pub fn label(self) -> &'static str {
        match self {
            DockSize::Small => "Small",
            DockSize::Medium => "Medium",
            DockSize::Large => "Large",
        }
    }

    /// Height of the bar itself, in points.
    pub fn bar_height(self) -> f32 {
        match self {
            DockSize::Small => 44.0,
            DockSize::Medium => 54.0,
            DockSize::Large => 66.0,
        }
    }

    /// Side of an app icon, in points.
    pub fn icon_size(self) -> f32 {
        match self {
            DockSize::Small => 26.0,
            DockSize::Medium => 34.0,
            DockSize::Large => 44.0,
        }
    }

    /// Base text size for widgets, in points.
    pub fn text_size(self) -> f32 {
        match self {
            DockSize::Small => 12.0,
            DockSize::Medium => 13.5,
            DockSize::Large => 15.5,
        }
    }
}

/// How the bar spreads along its edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockLayout {
    /// As wide as its contents, in the middle of the edge.
    #[default]
    Centred,
    /// Across the whole edge: apps on the left, widgets on the right.
    FullWidth,
}

impl DockLayout {
    pub const ALL: [DockLayout; 2] = [DockLayout::Centred, DockLayout::FullWidth];

    pub fn label(self) -> &'static str {
        match self {
            DockLayout::Centred => "Centred",
            DockLayout::FullWidth => "Full width",
        }
    }
}

/// How an app with open windows is marked under its icon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndicatorStyle {
    #[default]
    Dot,
    LargeDot,
    Line,
}

impl IndicatorStyle {
    pub const ALL: [IndicatorStyle; 3] = [
        IndicatorStyle::Dot,
        IndicatorStyle::LargeDot,
        IndicatorStyle::Line,
    ];

    pub fn label(self) -> &'static str {
        match self {
            IndicatorStyle::Dot => "Dot",
            IndicatorStyle::LargeDot => "Large dot",
            IndicatorStyle::Line => "Line",
        }
    }
}

/// What BarCraft does with the system taskbar while it runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskbarMode {
    /// Hide it completely; it comes back when BarCraft closes.
    #[default]
    Hidden,
    /// Only switch on auto-hide, so it still slides in at the screen edge.
    AutoHide,
    /// Leave it alone.
    Untouched,
}

impl TaskbarMode {
    pub const ALL: [TaskbarMode; 3] = [
        TaskbarMode::Hidden,
        TaskbarMode::AutoHide,
        TaskbarMode::Untouched,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TaskbarMode::Hidden => "Hide it",
            TaskbarMode::AutoHide => "Auto-hide it",
            TaskbarMode::Untouched => "Leave it as it is",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TempUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

/// Where the weather widget looks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WeatherSettings {
    /// The place name as the user typed it.
    pub city: String,
    /// The place the name resolved to, shown back to the user.
    pub resolved_name: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    #[serde(deserialize_with = "lenient::value")]
    pub unit: TempUnit,
}

/// The taskbar state BarCraft found before it changed anything, so it can put it back
/// even after a crash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTaskbar {
    pub autohide: bool,
}

/// Readers that never fail: a value this version doesn't understand (a widget or theme added
/// by a newer version, a hand-edited typo) falls back to its default instead of making the
/// whole file unreadable, which would lose the pinned apps and the saved taskbar state.
mod lenient {
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Deserializer};

    pub fn value<'de, D, T>(d: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: DeserializeOwned + Default,
    {
        let raw = serde_json::Value::deserialize(d)?;
        Ok(serde_json::from_value(raw).unwrap_or_default())
    }

    /// Keeps every entry it can read and skips the rest.
    pub fn list<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: DeserializeOwned,
    {
        let raw = serde_json::Value::deserialize(d)?;
        Ok(match raw {
            serde_json::Value::Array(items) => items
                .into_iter()
                .filter_map(|item| serde_json::from_value(item).ok())
                .collect(),
            _ => Vec::new(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(deserialize_with = "lenient::value")]
    pub edge: DockEdge,
    #[serde(deserialize_with = "lenient::value")]
    pub size: DockSize,
    #[serde(deserialize_with = "lenient::value")]
    pub layout: DockLayout,
    /// Icons near the pointer grow and push their neighbours aside.
    #[serde(deserialize_with = "lenient::value")]
    pub magnify_icons: bool,
    /// How much bigger the icon under the pointer gets, in percent (10 to 100).
    #[serde(deserialize_with = "lenient::value")]
    pub magnify_percent: u32,
    #[serde(deserialize_with = "lenient::value")]
    pub theme: ThemeId,
    #[serde(deserialize_with = "lenient::value")]
    pub clock_24h: bool,
    #[serde(deserialize_with = "lenient::value")]
    pub show_seconds: bool,
    #[serde(deserialize_with = "lenient::value")]
    pub indicator: IndicatorStyle,
    #[serde(deserialize_with = "lenient::value")]
    pub taskbar_mode: TaskbarMode,
    #[serde(deserialize_with = "lenient::value")]
    pub launch_at_startup: bool,
    #[serde(deserialize_with = "lenient::value")]
    pub hide_for_fullscreen: bool,
    /// Windows may use the space under the dock: maximised windows reach the screen edge and
    /// the dock floats over them, as on a Mac. Off: the dock keeps its strip to itself.
    #[serde(deserialize_with = "lenient::value")]
    pub float_over_windows: bool,
    /// The dock slides out of sight when not in use and comes back when the pointer reaches
    /// the screen edge. Implies that windows use the space under it.
    #[serde(deserialize_with = "lenient::value")]
    pub auto_hide: bool,
    #[serde(deserialize_with = "lenient::list")]
    pub widgets: Vec<WidgetKind>,
    #[serde(deserialize_with = "lenient::list")]
    pub pinned: Vec<PinnedApp>,
    /// Whether the taskbar's pinned apps were already copied into `pinned` once.
    #[serde(deserialize_with = "lenient::value")]
    pub pins_imported: bool,
    #[serde(deserialize_with = "lenient::value")]
    pub weather: WeatherSettings,
    #[serde(deserialize_with = "lenient::value")]
    pub saved_taskbar: Option<SavedTaskbar>,
    #[serde(deserialize_with = "lenient::value")]
    pub focus_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            edge: DockEdge::Bottom,
            size: DockSize::Medium,
            layout: DockLayout::Centred,
            magnify_icons: true,
            magnify_percent: DEFAULT_MAGNIFY_PERCENT,
            theme: ThemeId::FollowWindows,
            clock_24h: true,
            show_seconds: false,
            indicator: IndicatorStyle::Dot,
            taskbar_mode: TaskbarMode::Hidden,
            launch_at_startup: false,
            hide_for_fullscreen: true,
            float_over_windows: false,
            auto_hide: false,
            widgets: WidgetKind::DEFAULT_SET.to_vec(),
            pinned: Vec::new(),
            pins_imported: false,
            weather: WeatherSettings::default(),
            saved_taskbar: None,
            focus_minutes: 25,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not read or write the settings file: {0}")]
    Io(#[from] std::io::Error),
    #[error("the settings file is not valid: {0}")]
    Json(#[from] serde_json::Error),
}

/// How much the icon under the pointer grows unless the user picks otherwise, in percent.
pub const DEFAULT_MAGNIFY_PERCENT: u32 = 60;

/// The largest settings file we agree to read. A real one is a few kilobytes.
const MAX_SETTINGS_BYTES: u64 = 4 * 1024 * 1024;

impl Settings {
    /// Reads settings from `path`. A missing file gives the defaults.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        match std::fs::metadata(path) {
            Ok(meta) if meta.len() > MAX_SETTINGS_BYTES => {
                return Err(SettingsError::Io(std::io::Error::other(
                    "settings file is too large",
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.into()),
        }
        let text = std::fs::read_to_string(path)?;
        let mut settings: Settings = serde_json::from_str(&text)?;
        settings.sanitize();
        Ok(settings)
    }

    /// Writes settings to `path` through a temporary file, so a crash never leaves half a file.
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        let tmp: PathBuf = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Repairs values a hand-edited file could break.
    pub fn sanitize(&mut self) {
        let mut seen = Vec::new();
        self.widgets.retain(|w| {
            if seen.contains(w) {
                false
            } else {
                seen.push(*w);
                true
            }
        });
        self.focus_minutes = self.focus_minutes.clamp(1, 180);
        // An unreadable value reads as 0; anything else is kept within what the slider offers.
        self.magnify_percent = match self.magnify_percent {
            0 => DEFAULT_MAGNIFY_PERCENT,
            p => p.clamp(10, 100),
        };
        let mut keys = Vec::new();
        self.pinned.retain(|p| {
            let key = normalize(&p.path);
            if p.path.as_os_str().is_empty() || keys.contains(&key) {
                false
            } else {
                keys.push(key);
                true
            }
        });
        if self.pinned.len() > 64 {
            self.pinned.truncate(64);
        }
        if let (Some(lat), Some(lon)) = (self.weather.latitude, self.weather.longitude)
            && (!lat.is_finite()
                || !lon.is_finite()
                || !(-90.0..=90.0).contains(&lat)
                || !(-180.0..=180.0).contains(&lon))
        {
            self.weather.latitude = None;
            self.weather.longitude = None;
        }
    }

    pub fn has_widget(&self, kind: WidgetKind) -> bool {
        self.widgets.contains(&kind)
    }

    /// Adds the widget at the end, or removes it if it is already on the dock.
    pub fn toggle_widget(&mut self, kind: WidgetKind) {
        if let Some(i) = self.widgets.iter().position(|w| *w == kind) {
            self.widgets.remove(i);
        } else {
            self.widgets.push(kind);
        }
    }

    /// Moves a widget one place left (`-1`) or right (`+1`).
    pub fn move_widget(&mut self, kind: WidgetKind, delta: isize) {
        move_item(&mut self.widgets, |w| *w == kind, delta);
    }

    pub fn is_pinned(&self, path: &Path) -> bool {
        let key = normalize(path);
        self.pinned.iter().any(|p| normalize(&p.path) == key)
    }

    pub fn pin(&mut self, app: PinnedApp) {
        if !self.is_pinned(&app.path) {
            self.pinned.push(app);
        }
    }

    pub fn unpin(&mut self, path: &Path) {
        let key = normalize(path);
        self.pinned.retain(|p| normalize(&p.path) != key);
    }

    pub fn move_pin(&mut self, path: &Path, delta: isize) {
        let key = normalize(path);
        move_item(&mut self.pinned, |p| normalize(&p.path) == key, delta);
    }
}

fn normalize(path: &Path) -> String {
    crate::apps::normalize_path(path)
}

fn move_item<T>(items: &mut [T], is_it: impl Fn(&T) -> bool, delta: isize) {
    let Some(from) = items.iter().position(is_it) else {
        return;
    };
    let to = from
        .saturating_add_signed(delta)
        .min(items.len().saturating_sub(1));
    if to != from {
        items.swap(from, to);
    }
}

impl Settings {
    /// Whether the dock leaves its strip of the screen to other windows.
    pub fn windows_under_dock(&self) -> bool {
        self.float_over_windows || self.auto_hide
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnification_stays_within_the_slider() {
        for (raw, want) in [
            ("\"lots\"", DEFAULT_MAGNIFY_PERCENT),
            ("0", DEFAULT_MAGNIFY_PERCENT),
            ("3", 10),
            ("45", 45),
            ("900", 100),
        ] {
            let json = format!(r#"{{"magnify_percent": {raw}}}"#);
            let mut s: Settings = serde_json::from_str(&json).unwrap();
            s.sanitize();
            assert_eq!(s.magnify_percent, want, "{raw}");
        }
        assert_eq!(Settings::default().magnify_percent, DEFAULT_MAGNIFY_PERCENT);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = std::env::temp_dir().join("barcraft-test-missing");
        let s = Settings::load(&dir.join("nope.json")).unwrap();
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("barcraft-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut s = Settings {
            edge: DockEdge::Top,
            ..Settings::default()
        };
        s.weather.city = "Reggio Emilia".into();
        s.save(&path).unwrap();
        let back = Settings::load(&path).unwrap();
        assert_eq!(back, s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let s: Settings = serde_json::from_str(r#"{"edge":"Top","something_new":1}"#).unwrap();
        assert_eq!(s.edge, DockEdge::Top);
        assert_eq!(s.size, DockSize::Medium);
    }

    #[test]
    fn values_from_another_version_do_not_lose_the_rest() {
        // A widget and a theme this version doesn't know, and a wrongly typed number.
        let json = r#"{
            "theme": "SomeFutureTheme",
            "widgets": ["Clock", "SomeFutureWidget", "Cpu"],
            "focus_minutes": "lots",
            "pinned": [{"path": "C:\\Apps\\a.exe", "name": "A"}, {"broken": true}],
            "saved_taskbar": {"autohide": true}
        }"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.theme, ThemeId::default());
        assert_eq!(s.widgets, vec![WidgetKind::Clock, WidgetKind::Cpu]);
        assert_eq!(s.focus_minutes, 0);
        assert_eq!(s.pinned.len(), 1);
        assert_eq!(s.saved_taskbar, Some(SavedTaskbar { autohide: true }));
    }

    #[test]
    fn sanitize_removes_duplicates_and_bad_coordinates() {
        let mut s = Settings {
            widgets: vec![WidgetKind::Clock, WidgetKind::Clock, WidgetKind::Cpu],
            focus_minutes: 0,
            ..Settings::default()
        };
        s.weather.latitude = Some(f64::NAN);
        s.weather.longitude = Some(10.0);
        s.sanitize();
        assert_eq!(s.widgets, vec![WidgetKind::Clock, WidgetKind::Cpu]);
        assert_eq!(s.weather.latitude, None);
        assert_eq!(s.focus_minutes, 1);
    }

    #[test]
    fn toggle_and_move_widgets() {
        let mut s = Settings {
            widgets: vec![WidgetKind::Clock, WidgetKind::Cpu],
            ..Settings::default()
        };
        s.toggle_widget(WidgetKind::Memory);
        assert_eq!(s.widgets.last(), Some(&WidgetKind::Memory));
        s.move_widget(WidgetKind::Memory, -1);
        assert_eq!(
            s.widgets,
            vec![WidgetKind::Clock, WidgetKind::Memory, WidgetKind::Cpu]
        );
        s.move_widget(WidgetKind::Clock, -5);
        assert_eq!(s.widgets[0], WidgetKind::Clock);
        s.move_widget(WidgetKind::Cpu, 9);
        assert_eq!(s.widgets[2], WidgetKind::Cpu);
        s.toggle_widget(WidgetKind::Clock);
        assert!(!s.has_widget(WidgetKind::Clock));
    }

    #[test]
    fn pin_is_case_insensitive_and_unique() {
        let mut s = Settings::default();
        s.pin(PinnedApp::new("C:\\Apps\\Tool.exe", "Tool"));
        s.pin(PinnedApp::new("c:\\apps\\tool.EXE", "Tool"));
        assert_eq!(s.pinned.len(), 1);
        assert!(s.is_pinned(Path::new("C:/apps/tool.exe")));
        s.unpin(Path::new("C:\\APPS\\TOOL.exe"));
        assert!(s.pinned.is_empty());
    }
}
