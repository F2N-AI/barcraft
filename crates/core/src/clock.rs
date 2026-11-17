//! Small time helpers for widgets: how far through the day, month and year we are, a
//! stopwatch, and human-readable sizes and speeds. Dates go through `chrono`.

use std::time::{Duration, Instant};

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};

fn fraction(elapsed: i64, total: i64) -> f32 {
    if total <= 0 {
        return 0.0;
    }
    (elapsed as f64 / total as f64).clamp(0.0, 1.0) as f32
}

pub fn day_fraction(now: NaiveDateTime) -> f32 {
    fraction(i64::from(now.num_seconds_from_midnight()), 86_400)
}

pub fn month_fraction(now: NaiveDateTime) -> f32 {
    let Some(start) = NaiveDate::from_ymd_opt(now.year(), now.month(), 1) else {
        return 0.0;
    };
    let Some(next) = start.checked_add_months(chrono::Months::new(1)) else {
        return 0.0;
    };
    let total = next.signed_duration_since(start).num_seconds();
    let elapsed = now
        .signed_duration_since(start.and_time(chrono::NaiveTime::MIN))
        .num_seconds();
    fraction(elapsed, total)
}

pub fn year_fraction(now: NaiveDateTime) -> f32 {
    let (Some(start), Some(next)) = (
        NaiveDate::from_ymd_opt(now.year(), 1, 1),
        NaiveDate::from_ymd_opt(now.year().saturating_add(1), 1, 1),
    ) else {
        return 0.0;
    };
    let total = next.signed_duration_since(start).num_seconds();
    let elapsed = now
        .signed_duration_since(start.and_time(chrono::NaiveTime::MIN))
        .num_seconds();
    fraction(elapsed, total)
}

/// A stopwatch: start, pause, resume, reset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stopwatch {
    banked: Duration,
    since: Option<Instant>,
}

impl Stopwatch {
    pub fn elapsed(&self, now: Instant) -> Duration {
        let running = self
            .since
            .map(|s| now.saturating_duration_since(s))
            .unwrap_or_default();
        self.banked.saturating_add(running)
    }

    pub fn is_running(&self) -> bool {
        self.since.is_some()
    }

    pub fn toggle(&mut self, now: Instant) {
        match self.since.take() {
            Some(s) => self.banked = self.banked.saturating_add(now.saturating_duration_since(s)),
            None => self.since = Some(now),
        }
    }

    pub fn reset(&mut self) {
        *self = Stopwatch::default();
    }
}

/// `m:ss.d` under an hour, then `h:mm:ss`.
pub fn format_stopwatch(d: Duration) -> String {
    let secs = d.as_secs();
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}.{}", d.subsec_millis() / 100)
    }
}

/// `512 B`, `3.4 KB`, `12 MB`, `1.2 GB`, `2.0 TB` (powers of 1024, short names).
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    let name = UNITS.get(unit).copied().unwrap_or("B");
    if unit == 0 || value >= 100.0 {
        format!("{value:.0} {name}")
    } else {
        format!("{value:.1} {name}")
    }
}

/// A transfer speed: `0 B/s`, `340 KB/s`, `12 MB/s`.
pub fn format_rate(bytes_per_second: f64) -> String {
    let b = if bytes_per_second.is_finite() && bytes_per_second > 0.0 {
        bytes_per_second as u64
    } else {
        0
    };
    format!("{}/s", format_bytes(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, mo: u32, d: u32, h: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, mo, d)
            .unwrap()
            .and_hms_opt(h, 0, 0)
            .unwrap()
    }

    #[test]
    fn progress_fractions() {
        assert!((day_fraction(at(2026, 11, 17, 12)) - 0.5).abs() < 1e-6);
        // 16 full days of 30 in November.
        assert!((month_fraction(at(2026, 11, 17, 0)) - 16.0 / 30.0).abs() < 1e-6);
        assert_eq!(year_fraction(at(2026, 1, 1, 0)), 0.0);
        assert!(year_fraction(at(2026, 12, 31, 23)) > 0.99);
    }

    #[test]
    fn stopwatch_runs_and_pauses() {
        let t0 = Instant::now();
        let mut s = Stopwatch::default();
        s.toggle(t0);
        s.toggle(t0 + Duration::from_secs(5));
        assert_eq!(
            s.elapsed(t0 + Duration::from_secs(100)),
            Duration::from_secs(5)
        );
        s.toggle(t0 + Duration::from_secs(100));
        assert_eq!(
            s.elapsed(t0 + Duration::from_secs(102)),
            Duration::from_secs(7)
        );
        s.reset();
        assert!(!s.is_running());
    }

    #[test]
    fn formats() {
        assert_eq!(format_stopwatch(Duration::from_millis(65_300)), "1:05.3");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(3 * 1024 + 400), "3.4 KB");
        assert_eq!(format_bytes(500 * 1024 * 1024 * 1024), "500 GB");
        assert_eq!(format_rate(f64::NAN), "0 B/s");
    }
}
