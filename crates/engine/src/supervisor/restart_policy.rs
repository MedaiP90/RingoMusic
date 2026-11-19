// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Restarts that the supervisor allows in one window.
const MAX_RESTARTS: usize = 5;
/// Length of the sliding window.
const WINDOW: Duration = Duration::from_secs(5 * 60);
/// Delay before the first restart. Each restart doubles it.
const BACKOFF_START: Duration = Duration::from_secs(1);
/// Number of doublings after which the delay stops to grow (1, 2, 4, 8, 16 s).
const BACKOFF_MAX_DOUBLINGS: u32 = 4;

/// Returns the delay before restart number `attempt`, counted from 0.
pub(super) fn backoff_delay(attempt: u32) -> Duration {
    BACKOFF_START * (1 << attempt.min(BACKOFF_MAX_DOUBLINGS))
}

/// Counts restarts in a sliding window.
#[derive(Debug, Default)]
pub(super) struct RestartWindow {
    restarts: VecDeque<Instant>,
}

impl RestartWindow {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Records a restart at `now`. Returns `false` if the window is full.
    pub(super) fn try_record(&mut self, now: Instant) -> bool {
        while let Some(oldest) = self.restarts.front() {
            if now.saturating_duration_since(*oldest) < WINDOW {
                break;
            }
            self.restarts.pop_front();
        }
        if self.restarts.len() >= MAX_RESTARTS {
            return false;
        }
        self.restarts.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_and_stops_at_sixteen_seconds() {
        let delays: Vec<u64> = (0..7).map(|n| backoff_delay(n).as_secs()).collect();
        assert_eq!(delays, [1, 2, 4, 8, 16, 16, 16]);
    }

    #[test]
    fn window_allows_five_restarts_and_refuses_the_sixth() {
        let mut window = RestartWindow::new();
        let start = Instant::now();
        for second in 0..5 {
            assert!(window.try_record(start + Duration::from_secs(second)));
        }
        assert!(!window.try_record(start + Duration::from_secs(5)));
    }

    #[test]
    fn window_forgets_old_restarts() {
        let mut window = RestartWindow::new();
        let start = Instant::now();
        for _ in 0..5 {
            assert!(window.try_record(start));
        }
        assert!(!window.try_record(start + Duration::from_secs(299)));
        assert!(window.try_record(start + WINDOW));
    }
}
