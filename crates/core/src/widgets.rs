//! The catalogue of widgets the dock can show.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WidgetKind {
    Clock,
    AnalogClock,
    Date,
    DayProgress,
    MonthProgress,
    YearProgress,
    Cpu,
    CpuRing,
    Memory,
    MemoryRing,
    Network,
    Storage,
    Battery,
    BatteryTile,
    SystemTray,
    Notifications,
    RecycleBin,
    ShowDesktop,
    Volume,
    Media,
    Microphone,
    FocusTimer,
    Stopwatch,
    TimeOnPc,
    Weather,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetGroup {
    Time,
    System,
    Media,
    Productivity,
    Online,
}

impl WidgetGroup {
    pub const ALL: [WidgetGroup; 5] = [
        WidgetGroup::Time,
        WidgetGroup::System,
        WidgetGroup::Media,
        WidgetGroup::Productivity,
        WidgetGroup::Online,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WidgetGroup::Time => "Time and date",
            WidgetGroup::System => "System",
            WidgetGroup::Media => "Sound and media",
            WidgetGroup::Productivity => "Productivity",
            WidgetGroup::Online => "Online",
        }
    }
}

impl WidgetKind {
    pub const ALL: [WidgetKind; 25] = [
        WidgetKind::Clock,
        WidgetKind::AnalogClock,
        WidgetKind::Date,
        WidgetKind::DayProgress,
        WidgetKind::MonthProgress,
        WidgetKind::YearProgress,
        WidgetKind::Cpu,
        WidgetKind::CpuRing,
        WidgetKind::Memory,
        WidgetKind::MemoryRing,
        WidgetKind::Network,
        WidgetKind::Storage,
        WidgetKind::Battery,
        WidgetKind::BatteryTile,
        WidgetKind::SystemTray,
        WidgetKind::Notifications,
        WidgetKind::RecycleBin,
        WidgetKind::ShowDesktop,
        WidgetKind::Volume,
        WidgetKind::Media,
        WidgetKind::Microphone,
        WidgetKind::FocusTimer,
        WidgetKind::Stopwatch,
        WidgetKind::TimeOnPc,
        WidgetKind::Weather,
    ];

    /// What a fresh install shows.
    pub const DEFAULT_SET: [WidgetKind; 8] = [
        WidgetKind::Media,
        WidgetKind::Cpu,
        WidgetKind::Memory,
        WidgetKind::Volume,
        WidgetKind::Battery,
        WidgetKind::SystemTray,
        WidgetKind::Date,
        WidgetKind::Clock,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WidgetKind::Clock => "Clock",
            WidgetKind::AnalogClock => "Analog clock",
            WidgetKind::Date => "Date and calendar",
            WidgetKind::DayProgress => "Day progress",
            WidgetKind::MonthProgress => "Month progress",
            WidgetKind::YearProgress => "Year progress",
            WidgetKind::Cpu => "Processor load",
            WidgetKind::CpuRing => "Processor load (ring)",
            WidgetKind::Memory => "Memory in use",
            WidgetKind::MemoryRing => "Memory in use (ring)",
            WidgetKind::Network => "Network speed",
            WidgetKind::Storage => "Disk space",
            WidgetKind::Battery => "Battery",
            WidgetKind::BatteryTile => "Battery (tile)",
            WidgetKind::SystemTray => "System tray",
            WidgetKind::Notifications => "Notifications",
            WidgetKind::RecycleBin => "Recycle Bin",
            WidgetKind::ShowDesktop => "Show desktop",
            WidgetKind::Volume => "Volume",
            WidgetKind::Media => "Media",
            WidgetKind::Microphone => "Microphone",
            WidgetKind::FocusTimer => "Focus timer",
            WidgetKind::Stopwatch => "Stopwatch",
            WidgetKind::TimeOnPc => "Time on PC",
            WidgetKind::Weather => "Weather",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            WidgetKind::Clock => "The time, in 12- or 24-hour format.",
            WidgetKind::AnalogClock => "A clock face with hands.",
            WidgetKind::Date => "Today's weekday and date. Click it for a month calendar.",
            WidgetKind::DayProgress => "How much of today has gone by.",
            WidgetKind::MonthProgress => "How much of this month has gone by.",
            WidgetKind::YearProgress => "How much of this year has gone by.",
            WidgetKind::Cpu => "How busy the processor is, with the last minute as a chart.",
            WidgetKind::Memory => "How much memory is in use.",
            WidgetKind::CpuRing => "Processor load as a ring around the number, in a square tile.",
            WidgetKind::MemoryRing => {
                "Memory in use as a ring around the number, in a square tile."
            }
            WidgetKind::BatteryTile => {
                "A square tile that fills with the charge left. Hidden on PCs without a battery."
            }
            WidgetKind::Network => "Download and upload speed right now.",
            WidgetKind::Storage => "Free space on the drive Windows is on.",
            WidgetKind::Battery => {
                "Charge left and whether it is charging. Hidden on PCs without a battery."
            }
            WidgetKind::SystemTray => {
                "Brings up the Windows taskbar with its tray icons while the dock hides it."
            }
            WidgetKind::Notifications => "Opens the Windows notification centre.",
            WidgetKind::RecycleBin => {
                "What's in the Recycle Bin. Click to open it; empty it from its menu."
            }
            WidgetKind::ShowDesktop => "Minimises every window, or brings them back.",
            WidgetKind::Volume => "Speaker volume. Scroll to change it, click to mute.",
            WidgetKind::Media => "What's playing, with play/pause, next and previous.",
            WidgetKind::Microphone => "Shows whether the microphone is muted. Click to switch.",
            WidgetKind::FocusTimer => {
                "A countdown for focused work. Click to start or pause, right-click to reset."
            }
            WidgetKind::Stopwatch => "Click to start or pause, right-click to reset.",
            WidgetKind::TimeOnPc => {
                "How long you've used the PC today, and the apps you used most. Stays on this PC."
            }
            WidgetKind::Weather => {
                "Temperature, sky and the next days for a place you choose. Uses Open-Meteo."
            }
        }
    }

    pub fn group(self) -> WidgetGroup {
        match self {
            WidgetKind::Clock
            | WidgetKind::AnalogClock
            | WidgetKind::Date
            | WidgetKind::DayProgress
            | WidgetKind::MonthProgress
            | WidgetKind::YearProgress => WidgetGroup::Time,
            WidgetKind::Cpu
            | WidgetKind::CpuRing
            | WidgetKind::Memory
            | WidgetKind::MemoryRing
            | WidgetKind::Network
            | WidgetKind::Storage
            | WidgetKind::Battery
            | WidgetKind::BatteryTile
            | WidgetKind::SystemTray
            | WidgetKind::Notifications
            | WidgetKind::RecycleBin
            | WidgetKind::ShowDesktop => WidgetGroup::System,
            WidgetKind::Volume | WidgetKind::Media | WidgetKind::Microphone => WidgetGroup::Media,
            WidgetKind::FocusTimer | WidgetKind::Stopwatch | WidgetKind::TimeOnPc => {
                WidgetGroup::Productivity
            }
            WidgetKind::Weather => WidgetGroup::Online,
        }
    }

    /// Whether the widget contacts a service on the internet.
    pub fn goes_online(self) -> bool {
        matches!(self, WidgetKind::Weather)
    }

    /// Whether the widget keeps a record of what the user does (shown in Privacy settings).
    pub fn records_activity(self) -> bool {
        matches!(self, WidgetKind::TimeOnPc)
    }

    /// Whether `query` matches the widget's name or description, ignoring case.
    pub fn matches(self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.is_empty()
            || self.label().to_lowercase().contains(&q)
            || self.description().to_lowercase().contains(&q)
            || self.group().label().to_lowercase().contains(&q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_set_has_no_duplicates_and_stays_offline() {
        for (i, a) in WidgetKind::DEFAULT_SET.iter().enumerate() {
            assert!(!a.goes_online());
            assert!(!a.records_activity());
            for b in WidgetKind::DEFAULT_SET.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn every_widget_is_listed_once_and_described() {
        for (i, k) in WidgetKind::ALL.iter().enumerate() {
            assert!(!k.label().is_empty());
            assert!(k.description().ends_with('.'));
            assert!(!WidgetKind::ALL[i + 1..].contains(k), "{k:?} listed twice");
        }
    }

    #[test]
    fn search_matches_name_description_and_group() {
        assert!(WidgetKind::Network.matches("download"));
        assert!(WidgetKind::Clock.matches("TIME"));
        assert!(WidgetKind::Clock.matches(""));
        assert!(!WidgetKind::Clock.matches("battery"));
    }
}
