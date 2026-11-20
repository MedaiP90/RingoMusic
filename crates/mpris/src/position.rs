// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::{Duration, Instant};

use ringo_engine::ItemId;

use crate::mapping::to_time;

/// A position that differs from the expected one by more than this is a seek.
const JUMP_TOLERANCE: Duration = Duration::from_secs(2);

/// Returns the position that a relative seek of `offset_micros` reaches.
///
/// A target before the start gives the start. A target after the end gives
/// `None`: the MPRIS specification says that this acts like "next".
pub(crate) fn seek_target(
    position: Duration,
    offset_micros: i64,
    duration: Duration,
) -> Option<Duration> {
    let current = to_time(position).as_micros();
    let target = current.saturating_add(offset_micros).max(0);
    if target > to_time(duration).as_micros() {
        return None;
    }
    Some(Duration::from_micros(target.unsigned_abs()))
}

/// Finds the position changes that clients cannot predict. Clients assume
/// that the position moves with the clock while an item plays.
#[derive(Debug, Default)]
pub(crate) struct JumpDetector {
    last: Option<Observation>,
}

#[derive(Debug)]
struct Observation {
    item: Option<ItemId>,
    position: Duration,
    playing: bool,
    at: Instant,
}

impl JumpDetector {
    /// Records the latest state. Returns the new position if it is a jump in
    /// the same item. A change of the item is not a jump.
    pub(crate) fn observe(
        &mut self,
        item: Option<&ItemId>,
        position: Duration,
        playing: bool,
        now: Instant,
    ) -> Option<Duration> {
        let previous = self.last.replace(Observation {
            item: item.cloned(),
            position,
            playing,
            at: now,
        })?;
        if previous.item.as_ref() != item {
            return None;
        }
        let expected = if previous.playing {
            previous.position + now.saturating_duration_since(previous.at)
        } else {
            previous.position
        };
        (position.abs_diff(expected) > JUMP_TOLERANCE).then_some(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn seek_target_adds_the_offset() {
        let target = seek_target(10 * SECOND, 5_000_000, 100 * SECOND);
        assert_eq!(target, Some(15 * SECOND));
    }

    #[test]
    fn seek_target_stops_at_the_start() {
        let target = seek_target(3 * SECOND, -10_000_000, 100 * SECOND);
        assert_eq!(target, Some(Duration::ZERO));
    }

    #[test]
    fn seek_target_is_none_after_the_end() {
        assert_eq!(seek_target(98 * SECOND, 5_000_000, 100 * SECOND), None);
    }

    #[test]
    fn seek_target_accepts_the_end() {
        let target = seek_target(98 * SECOND, 2_000_000, 100 * SECOND);
        assert_eq!(target, Some(100 * SECOND));
    }

    fn id(value: &str) -> ItemId {
        ItemId::new(value)
    }

    #[test]
    fn first_observation_is_not_a_jump() {
        let mut detector = JumpDetector::default();
        assert_eq!(
            detector.observe(Some(&id("1")), SECOND, true, Instant::now()),
            None
        );
    }

    #[test]
    fn normal_progress_is_not_a_jump() {
        let mut detector = JumpDetector::default();
        let start = Instant::now();
        detector.observe(Some(&id("1")), 10 * SECOND, true, start);
        let result = detector.observe(Some(&id("1")), 11 * SECOND, true, start + SECOND);
        assert_eq!(result, None);
    }

    #[test]
    fn forward_seek_while_playing_is_a_jump() {
        let mut detector = JumpDetector::default();
        let start = Instant::now();
        detector.observe(Some(&id("1")), 10 * SECOND, true, start);
        let result = detector.observe(Some(&id("1")), 60 * SECOND, true, start + SECOND);
        assert_eq!(result, Some(60 * SECOND));
    }

    #[test]
    fn position_change_while_paused_is_a_jump() {
        let mut detector = JumpDetector::default();
        let start = Instant::now();
        detector.observe(Some(&id("1")), 10 * SECOND, false, start);
        let result = detector.observe(Some(&id("1")), 30 * SECOND, false, start + SECOND);
        assert_eq!(result, Some(30 * SECOND));
    }

    #[test]
    fn restart_of_the_same_item_is_a_jump() {
        let mut detector = JumpDetector::default();
        let start = Instant::now();
        detector.observe(Some(&id("1")), 190 * SECOND, true, start);
        let result = detector.observe(Some(&id("1")), Duration::ZERO, true, start + SECOND);
        assert_eq!(result, Some(Duration::ZERO));
    }

    #[test]
    fn change_of_item_is_not_a_jump() {
        let mut detector = JumpDetector::default();
        let start = Instant::now();
        detector.observe(Some(&id("1")), 190 * SECOND, true, start);
        let result = detector.observe(Some(&id("2")), Duration::ZERO, true, start + SECOND);
        assert_eq!(result, None);
    }
}
