//! The catalogue of widgets the dock can show.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WidgetKind {
    Clock,
    Date,
    Cpu,
    Memory,
    Battery,
    Volume,
    Weather,
    FocusTimer,
    ShowDesktop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetGroup {
    Time,
    System,
    Media,
    Online,
    Productivity,
}

impl WidgetGroup {
    pub fn label(self) -> &'static str {
        match self {
            WidgetGroup::Time => "Time",
            WidgetGroup::System => "System",
            WidgetGroup::Media => "Sound",
            WidgetGroup::Online => "Online",
            WidgetGroup::Productivity => "Productivity",
        }
    }
}

impl WidgetKind {
    pub const ALL: [WidgetKind; 9] = [
        WidgetKind::Clock,
        WidgetKind::Date,
        WidgetKind::Cpu,
        WidgetKind::Memory,
        WidgetKind::Battery,
        WidgetKind::Volume,
        WidgetKind::Weather,
        WidgetKind::FocusTimer,
        WidgetKind::ShowDesktop,
    ];

    /// What a fresh install shows.
    pub const DEFAULT_SET: [WidgetKind; 6] = [
        WidgetKind::Cpu,
        WidgetKind::Memory,
        WidgetKind::Volume,
        WidgetKind::Battery,
        WidgetKind::Date,
        WidgetKind::Clock,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WidgetKind::Clock => "Clock",
            WidgetKind::Date => "Date and calendar",
            WidgetKind::Cpu => "Processor load",
            WidgetKind::Memory => "Memory in use",
            WidgetKind::Battery => "Battery",
            WidgetKind::Volume => "Volume",
            WidgetKind::Weather => "Weather",
            WidgetKind::FocusTimer => "Focus timer",
            WidgetKind::ShowDesktop => "Show desktop",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            WidgetKind::Clock => "The time, in 12- or 24-hour format.",
            WidgetKind::Date => "Today's date. Click it for a month calendar.",
            WidgetKind::Cpu => "How busy the processor is, with the last minute as a small chart.",
            WidgetKind::Memory => "How much memory is in use.",
            WidgetKind::Battery => {
                "Charge left and whether it is charging. Hidden on PCs without a battery."
            }
            WidgetKind::Volume => "Speaker volume. Scroll to change it, click to mute.",
            WidgetKind::Weather => {
                "Temperature and sky for a place you choose. Uses Open-Meteo, no account."
            }
            WidgetKind::FocusTimer => {
                "A countdown for focused work. Click to start or pause, right-click to reset."
            }
            WidgetKind::ShowDesktop => "Minimises every window, or brings them back.",
        }
    }

    pub fn group(self) -> WidgetGroup {
        match self {
            WidgetKind::Clock | WidgetKind::Date => WidgetGroup::Time,
            WidgetKind::Cpu
            | WidgetKind::Memory
            | WidgetKind::Battery
            | WidgetKind::ShowDesktop => WidgetGroup::System,
            WidgetKind::Volume => WidgetGroup::Media,
            WidgetKind::Weather => WidgetGroup::Online,
            WidgetKind::FocusTimer => WidgetGroup::Productivity,
        }
    }

    /// Whether the widget contacts a service on the internet.
    pub fn goes_online(self) -> bool {
        matches!(self, WidgetKind::Weather)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_set_has_no_duplicates_and_stays_offline() {
        for (i, a) in WidgetKind::DEFAULT_SET.iter().enumerate() {
            assert!(!a.goes_online());
            for b in WidgetKind::DEFAULT_SET.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn every_widget_is_described() {
        for k in WidgetKind::ALL {
            assert!(!k.label().is_empty());
            assert!(k.description().ends_with('.'));
        }
    }
}
