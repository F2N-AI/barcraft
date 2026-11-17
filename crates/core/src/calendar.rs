//! The month grid behind the calendar popup. Dates go through `chrono`.

use chrono::{Datelike, Months, NaiveDate};

/// One month laid out in weeks that start on Monday.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonthGrid {
    pub year: i32,
    pub month: u32,
    /// Each week holds seven slots, Monday first; `None` pads days outside the month.
    pub weeks: Vec<[Option<u32>; 7]>,
}

pub const WEEKDAY_SHORT: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

pub fn month_name(month: u32) -> &'static str {
    month
        .checked_sub(1)
        .and_then(|i| MONTH_NAMES.get(i as usize))
        .copied()
        .unwrap_or("")
}

/// Builds the grid for `year`-`month`, or `None` for a month chrono can't represent.
pub fn month_grid(year: i32, month: u32) -> Option<MonthGrid> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let next = first.checked_add_months(Months::new(1))?;
    let days_in_month = u32::try_from(next.signed_duration_since(first).num_days()).ok()?;
    let lead = first.weekday().num_days_from_monday() as usize;

    let mut weeks = Vec::with_capacity(6);
    let mut week = [None; 7];
    let mut slot = lead;
    for day in 1..=days_in_month {
        if let Some(cell) = week.get_mut(slot) {
            *cell = Some(day);
        }
        slot += 1;
        if slot == 7 {
            weeks.push(week);
            week = [None; 7];
            slot = 0;
        }
    }
    if slot > 0 {
        weeks.push(week);
    }
    Some(MonthGrid { year, month, weeks })
}

/// The month `delta` months away from `year`-`month`.
pub fn shift_month(year: i32, month: u32, delta: i32) -> Option<(i32, u32)> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let moved = if delta >= 0 {
        first.checked_add_months(Months::new(delta.unsigned_abs()))?
    } else {
        first.checked_sub_months(Months::new(delta.unsigned_abs()))?
    };
    Some((moved.year(), moved.month()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn november_2026_starts_on_sunday() {
        let g = month_grid(2026, 11).unwrap();
        assert_eq!(g.weeks[0], [None, None, None, None, None, None, Some(1)]);
        assert_eq!(g.weeks.last().unwrap()[0], Some(30));
        assert_eq!(g.weeks.len(), 6);
    }

    #[test]
    fn leap_february() {
        let g = month_grid(2028, 2).unwrap();
        let days: Vec<u32> = g.weeks.iter().flatten().flatten().copied().collect();
        assert_eq!(days.len(), 29);
    }

    #[test]
    fn invalid_month_is_none() {
        assert!(month_grid(2026, 13).is_none());
        assert!(month_grid(2026, 0).is_none());
        assert_eq!(month_name(0), "");
    }

    #[test]
    fn shifting_crosses_years() {
        assert_eq!(shift_month(2026, 12, 1), Some((2027, 1)));
        assert_eq!(shift_month(2026, 1, -1), Some((2025, 12)));
        assert_eq!(shift_month(2026, 5, 0), Some((2026, 5)));
    }
}
