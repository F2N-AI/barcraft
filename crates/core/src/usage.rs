//! "Time on PC": how long each app was in front today. Kept on the user's PC only.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The most apps remembered for one day; the rest are added to "Other".
const MAX_APPS: usize = 200;
const OTHER: &str = "Other";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DayUsage {
    /// The local date these totals belong to, as `YYYY-MM-DD`.
    pub date: String,
    /// Seconds in front, by app name.
    pub seconds: BTreeMap<String, u64>,
}

impl DayUsage {
    /// Adds time to an app, starting a fresh day when `date` changed.
    pub fn add(&mut self, date: &str, app: &str, secs: u64) {
        if self.date != date {
            self.date = date.to_owned();
            self.seconds.clear();
        }
        if secs == 0 || app.is_empty() {
            return;
        }
        let key = if self.seconds.contains_key(app) || self.seconds.len() < MAX_APPS {
            app
        } else {
            OTHER
        };
        let entry = self.seconds.entry(key.to_owned()).or_insert(0);
        *entry = entry.saturating_add(secs);
    }

    pub fn total(&self) -> u64 {
        self.seconds
            .values()
            .fold(0u64, |a, &b| a.saturating_add(b))
    }

    /// The `n` apps used longest, longest first.
    pub fn top(&self, n: usize) -> Vec<(&str, u64)> {
        let mut all: Vec<(&str, u64)> =
            self.seconds.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        all.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        all.truncate(n);
        all
    }

    pub fn load(path: &Path) -> DayUsage {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_string(self).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(tmp, path)
    }
}

/// `1h 05m`, `12m`, or `<1m`.
pub fn format_duration(secs: u64) -> String {
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    match (h, m) {
        (0, 0) => "<1m".to_owned(),
        (0, m) => format!("{m}m"),
        (h, m) => format!("{h}h {m:02}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_ranks() {
        let mut u = DayUsage::default();
        u.add("2026-11-17", "Editor", 120);
        u.add("2026-11-17", "Browser", 600);
        u.add("2026-11-17", "Editor", 60);
        assert_eq!(u.total(), 780);
        assert_eq!(u.top(1), vec![("Browser", 600)]);
    }

    #[test]
    fn new_day_starts_over() {
        let mut u = DayUsage::default();
        u.add("2026-11-17", "Editor", 120);
        u.add("2026-11-18", "Browser", 5);
        assert_eq!(u.total(), 5);
        assert_eq!(u.date, "2026-11-18");
    }

    #[test]
    fn app_count_is_bounded() {
        let mut u = DayUsage::default();
        for i in 0..(MAX_APPS + 50) {
            u.add("d", &format!("app{i}"), 1);
        }
        assert!(u.seconds.len() <= MAX_APPS + 1);
        assert_eq!(u.total(), (MAX_APPS + 50) as u64);
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(30), "<1m");
        assert_eq!(format_duration(12 * 60), "12m");
        assert_eq!(format_duration(3900), "1h 05m");
    }
}
