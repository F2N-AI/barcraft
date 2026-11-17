//! Processor load from two readings of the system's cumulative CPU times.

/// Cumulative processor times since boot, in any consistent unit.
/// `kernel` includes idle time, as Windows reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuTimes {
    pub idle: u64,
    pub kernel: u64,
    pub user: u64,
}

/// Fraction of time the processor was busy between two readings, from 0 to 1.
/// Returns `None` when the readings can't be compared (same instant, or the counters went back).
pub fn busy_fraction(before: CpuTimes, after: CpuTimes) -> Option<f32> {
    let idle = after.idle.checked_sub(before.idle)?;
    let kernel = after.kernel.checked_sub(before.kernel)?;
    let user = after.user.checked_sub(before.user)?;
    let total = kernel.checked_add(user)?;
    if total == 0 {
        return None;
    }
    let busy = total.saturating_sub(idle);
    Some((busy as f64 / total as f64).clamp(0.0, 1.0) as f32)
}

/// A fixed-length history of samples for the small charts.
#[derive(Clone, Debug)]
pub struct History {
    samples: std::collections::VecDeque<f32>,
    capacity: usize,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: std::collections::VecDeque::with_capacity(capacity),
            capacity: capacity.max(1),
        }
    }

    pub fn push(&mut self, value: f32) {
        let value = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(value);
    }

    pub fn latest(&self) -> Option<f32> {
        self.samples.back().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = f32> + '_ {
        self.samples.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_busy() {
        let a = CpuTimes {
            idle: 100,
            kernel: 200,
            user: 100,
        };
        let b = CpuTimes {
            idle: 150,
            kernel: 260,
            user: 140,
        };
        // total 100, idle 50
        assert_eq!(busy_fraction(a, b), Some(0.5));
    }

    #[test]
    fn counters_going_back_give_none() {
        let a = CpuTimes {
            idle: 100,
            kernel: 200,
            user: 100,
        };
        let b = CpuTimes {
            idle: 90,
            kernel: 260,
            user: 140,
        };
        assert_eq!(busy_fraction(a, b), None);
        assert_eq!(busy_fraction(a, a), None);
    }

    #[test]
    fn history_is_bounded_and_clamped() {
        let mut h = History::new(3);
        for v in [0.1, 2.0, f32::NAN, 0.4] {
            h.push(v);
        }
        assert_eq!(h.iter().collect::<Vec<_>>(), vec![1.0, 0.0, 0.4]);
        assert_eq!(h.latest(), Some(0.4));
    }
}
