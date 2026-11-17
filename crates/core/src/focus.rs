//! A countdown for focused work: start, pause, resume, reset.

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    Running { since: Instant, before: Duration },
    Paused { elapsed: Duration },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusTimer {
    length: Duration,
    state: State,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusPhase {
    Idle,
    Running,
    Paused,
    Finished,
}

impl FocusTimer {
    pub fn new(length: Duration) -> Self {
        Self {
            length,
            state: State::Idle,
        }
    }

    pub fn length(&self) -> Duration {
        self.length
    }

    /// Changes the length; a running countdown starts over.
    pub fn set_length(&mut self, length: Duration) {
        if length != self.length {
            self.length = length;
            self.state = State::Idle;
        }
    }

    fn elapsed(&self, now: Instant) -> Duration {
        match self.state {
            State::Idle => Duration::ZERO,
            State::Running { since, before } => {
                before.saturating_add(now.saturating_duration_since(since))
            }
            State::Paused { elapsed } => elapsed,
        }
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        self.length.saturating_sub(self.elapsed(now))
    }

    pub fn phase(&self, now: Instant) -> FocusPhase {
        match self.state {
            State::Idle => FocusPhase::Idle,
            _ if self.remaining(now).is_zero() => FocusPhase::Finished,
            State::Running { .. } => FocusPhase::Running,
            State::Paused { .. } => FocusPhase::Paused,
        }
    }

    /// Start, pause or resume. A finished countdown starts again.
    pub fn toggle(&mut self, now: Instant) {
        self.state = match (self.phase(now), self.state) {
            (FocusPhase::Finished, _) | (_, State::Idle) => State::Running {
                since: now,
                before: Duration::ZERO,
            },
            (_, State::Running { .. }) => State::Paused {
                elapsed: self.elapsed(now),
            },
            (_, State::Paused { elapsed }) => State::Running {
                since: now,
                before: elapsed,
            },
        };
    }

    pub fn reset(&mut self) {
        self.state = State::Idle;
    }
}

/// `mm:ss`, or `h:mm:ss` from an hour up.
pub fn format_countdown(d: Duration) -> String {
    let secs = d.as_secs();
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_pauses_and_finishes() {
        let t0 = Instant::now();
        let mut t = FocusTimer::new(Duration::from_secs(60));
        assert_eq!(t.phase(t0), FocusPhase::Idle);
        t.toggle(t0);
        assert_eq!(
            t.remaining(t0 + Duration::from_secs(20)),
            Duration::from_secs(40)
        );
        t.toggle(t0 + Duration::from_secs(20));
        assert_eq!(t.phase(t0 + Duration::from_secs(500)), FocusPhase::Paused);
        assert_eq!(
            t.remaining(t0 + Duration::from_secs(500)),
            Duration::from_secs(40)
        );
        t.toggle(t0 + Duration::from_secs(500));
        assert_eq!(t.phase(t0 + Duration::from_secs(540)), FocusPhase::Finished);
        t.toggle(t0 + Duration::from_secs(541));
        assert_eq!(
            t.remaining(t0 + Duration::from_secs(541)),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn formats() {
        assert_eq!(format_countdown(Duration::from_secs(25 * 60)), "25:00");
        assert_eq!(format_countdown(Duration::from_secs(3725)), "1:02:05");
    }
}
