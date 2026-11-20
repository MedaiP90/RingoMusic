// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use rand::SeedableRng;
use rand::rngs::StdRng;
use ringo_engine::ItemId;
use ringo_engine::protocol::RepeatMode;

use super::{Queue, QueueChange, QueueItem};

fn item(id: &str) -> QueueItem {
    QueueItem {
        id: ItemId::new(id),
        title: format!("title {id}"),
        artist: "artist".to_owned(),
        album: "album".to_owned(),
        duration: Duration::from_secs(180),
        artwork_template: None,
    }
}

fn items(ids: &[&str]) -> Vec<QueueItem> {
    ids.iter().map(|id| item(id)).collect()
}

fn queue(ids: &[&str], start: usize) -> Queue {
    let mut queue = Queue::new();
    let _ = queue.replace(items(ids), start);
    queue
}

fn rng() -> StdRng {
    StdRng::seed_from_u64(7)
}

fn user_order(queue: &Queue) -> Vec<&str> {
    queue.items().iter().map(|item| item.id.as_str()).collect()
}

fn playback(queue: &Queue) -> Vec<String> {
    queue
        .playback_order()
        .into_iter()
        .map(ItemId::into_string)
        .collect()
}

fn current_id(queue: &Queue) -> &str {
    queue
        .current()
        .expect("queue has a current item")
        .id
        .as_str()
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

#[test]
fn replace_clamps_start_and_turns_shuffle_off() {
    let mut queue = queue(&["a", "b", "c"], 0);
    let _ = queue.set_shuffle(true, &mut rng());

    assert_eq!(queue.replace(items(&["x", "y"]), 9), QueueChange::Replaced);

    assert_eq!(current_id(&queue), "y");
    assert!(!queue.shuffle_enabled());
}

#[test]
fn replace_with_no_items_leaves_no_current_item() {
    let mut queue = queue(&["a"], 0);

    assert_eq!(queue.replace(Vec::new(), 0), QueueChange::Replaced);

    assert!(queue.current().is_none());
    assert_eq!(queue.current_playback_index(), None);
}

#[test]
fn append_to_empty_queue_starts_at_first_item() {
    let mut queue = Queue::new();

    assert_eq!(queue.append(items(&["a", "b"])), QueueChange::Replaced);

    assert_eq!(current_id(&queue), "a");
}

#[test]
fn append_keeps_current_item_and_reports_upcoming_only() {
    let mut queue = queue(&["a", "b"], 1);

    assert_eq!(queue.append(items(&["c"])), QueueChange::UpcomingOnly);

    assert_eq!(current_id(&queue), "b");
    assert_eq!(user_order(&queue), ["a", "b", "c"]);
}

#[test]
fn append_nothing_changes_nothing() {
    let mut queue = queue(&["a"], 0);

    assert_eq!(queue.append(Vec::new()), QueueChange::None);
}

#[test]
fn append_in_shuffle_mode_goes_to_end_of_playback_order() {
    let mut queue = queue(&["a", "b", "c"], 1);
    let _ = queue.set_shuffle(true, &mut rng());

    let _ = queue.append(items(&["x", "y"]));

    let order = playback(&queue);
    assert_eq!(order[0], "b");
    assert_eq!(order[3..], ["x", "y"]);
}

#[test]
fn play_next_inserts_after_current_item() {
    let mut queue = queue(&["a", "b", "c"], 1);

    assert_eq!(
        queue.play_next(items(&["x", "y"])),
        QueueChange::UpcomingOnly
    );

    assert_eq!(user_order(&queue), ["a", "b", "x", "y", "c"]);
    assert_eq!(current_id(&queue), "b");
}

#[test]
fn play_next_in_shuffle_mode_plays_new_items_next() {
    let mut queue = queue(&["a", "b", "c", "d"], 2);
    let _ = queue.set_shuffle(true, &mut rng());

    let _ = queue.play_next(items(&["x", "y"]));

    let order = playback(&queue);
    assert_eq!(order[..3], ["c", "x", "y"]);
    assert_eq!(sorted(order), ["a", "b", "c", "d", "x", "y"]);
}

#[test]
fn play_next_on_empty_queue_acts_like_append() {
    let mut queue = Queue::new();

    assert_eq!(queue.play_next(items(&["a"])), QueueChange::Replaced);

    assert_eq!(current_id(&queue), "a");
}

#[test]
fn remove_before_current_item_keeps_current_item() {
    let mut queue = queue(&["a", "b", "c"], 2);

    assert_eq!(queue.remove(0), QueueChange::UpcomingOnly);

    assert_eq!(current_id(&queue), "c");
    assert_eq!(queue.current_playback_index(), Some(1));
}

#[test]
fn remove_current_item_selects_next_item() {
    let mut queue = queue(&["a", "b", "c"], 1);

    assert_eq!(queue.remove(1), QueueChange::Replaced);

    assert_eq!(current_id(&queue), "c");
}

#[test]
fn remove_last_current_item_selects_previous_item() {
    let mut queue = queue(&["a", "b", "c"], 2);

    assert_eq!(queue.remove(2), QueueChange::Replaced);

    assert_eq!(current_id(&queue), "b");
}

#[test]
fn remove_current_item_in_shuffle_mode_selects_next_in_playback_order() {
    let mut queue = queue(&["a", "b", "c", "d"], 0);
    let _ = queue.set_shuffle(true, &mut rng());
    let expected = playback(&queue)[1].clone();

    let _ = queue.remove(0);

    assert_eq!(current_id(&queue), expected);
    assert_eq!(queue.items().len(), 3);
    assert_eq!(queue.playback_order().len(), 3);
}

#[test]
fn remove_only_item_empties_queue() {
    let mut queue = queue(&["a"], 0);

    assert_eq!(queue.remove(0), QueueChange::Replaced);

    assert!(queue.current().is_none());
    assert!(queue.items().is_empty());
}

#[test]
fn remove_with_invalid_index_changes_nothing() {
    let mut queue = queue(&["a"], 0);

    assert_eq!(queue.remove(5), QueueChange::None);
    assert_eq!(Queue::new().remove(0), QueueChange::None);
}

#[test]
fn move_item_across_current_item_keeps_current_item() {
    let mut queue = queue(&["a", "b", "c", "d"], 1);

    assert_eq!(queue.move_item(0, 2), QueueChange::UpcomingOnly);

    assert_eq!(user_order(&queue), ["b", "c", "a", "d"]);
    assert_eq!(current_id(&queue), "b");
    assert_eq!(queue.current_playback_index(), Some(0));
}

#[test]
fn move_item_backwards_across_current_item_keeps_current_item() {
    let mut queue = queue(&["a", "b", "c", "d"], 1);

    let _ = queue.move_item(3, 0);

    assert_eq!(user_order(&queue), ["d", "a", "b", "c"]);
    assert_eq!(current_id(&queue), "b");
}

#[test]
fn move_current_item_keeps_it_current() {
    let mut queue = queue(&["a", "b", "c"], 0);

    let _ = queue.move_item(0, 2);

    assert_eq!(current_id(&queue), "a");
    assert_eq!(queue.current_playback_index(), Some(2));
}

#[test]
fn move_item_in_shuffle_mode_keeps_playback_order_of_other_items() {
    let mut queue = queue(&["a", "b", "c", "d", "e"], 2);
    let _ = queue.set_shuffle(true, &mut rng());
    let before = playback(&queue);

    let _ = queue.move_item(0, 4);

    assert_eq!(playback(&queue), before);
    assert_eq!(current_id(&queue), "c");
}

#[test]
fn move_item_with_invalid_indexes_changes_nothing() {
    let mut queue = queue(&["a", "b"], 0);

    assert_eq!(queue.move_item(0, 0), QueueChange::None);
    assert_eq!(queue.move_item(0, 2), QueueChange::None);
    assert_eq!(queue.move_item(2, 0), QueueChange::None);
}

#[test]
fn clear_removes_items_and_keeps_shuffle_setting() {
    let mut queue = queue(&["a", "b"], 0);
    let _ = queue.set_shuffle(true, &mut rng());

    assert_eq!(queue.clear(), QueueChange::Replaced);
    assert_eq!(queue.clear(), QueueChange::None);

    assert!(queue.shuffle_enabled());
    assert_eq!(queue.append(items(&["x", "y"])), QueueChange::Replaced);
    assert_eq!(playback(&queue), ["x", "y"]);
}

#[test]
fn shuffle_keeps_current_item_first_and_all_items() {
    let mut queue = queue(&["a", "b", "c", "d", "e", "f"], 3);

    assert_eq!(
        queue.set_shuffle(true, &mut rng()),
        QueueChange::UpcomingOnly
    );

    let order = playback(&queue);
    assert_eq!(order[0], "d");
    assert_eq!(sorted(order), ["a", "b", "c", "d", "e", "f"]);
    assert_eq!(queue.current_playback_index(), Some(0));
    assert_eq!(user_order(&queue), ["a", "b", "c", "d", "e", "f"]);
}

#[test]
fn shuffle_is_the_same_for_the_same_seed() {
    let mut first = queue(&["a", "b", "c", "d", "e", "f"], 0);
    let mut second = first.clone();

    let _ = first.set_shuffle(true, &mut rng());
    let _ = second.set_shuffle(true, &mut rng());

    assert_eq!(playback(&first), playback(&second));
}

#[test]
fn shuffle_off_returns_to_user_order_at_current_item() {
    let mut queue = queue(&["a", "b", "c", "d", "e"], 0);
    let _ = queue.set_shuffle(true, &mut rng());
    let _ = queue.advance();
    let current = current_id(&queue).to_owned();

    assert_eq!(
        queue.set_shuffle(false, &mut rng()),
        QueueChange::UpcomingOnly
    );

    assert_eq!(playback(&queue), ["a", "b", "c", "d", "e"]);
    assert_eq!(current_id(&queue), current);
}

#[test]
fn shuffle_with_one_item_is_valid() {
    let mut queue = queue(&["a"], 0);

    let _ = queue.set_shuffle(true, &mut rng());

    assert_eq!(playback(&queue), ["a"]);
    assert_eq!(queue.advance(), None);
}

#[test]
fn shuffle_on_empty_queue_reports_no_change() {
    let mut queue = Queue::new();

    assert_eq!(queue.set_shuffle(true, &mut rng()), QueueChange::None);
    assert!(queue.shuffle_enabled());
    assert_eq!(queue.set_shuffle(true, &mut rng()), QueueChange::None);
}

#[test]
fn advance_without_repeat_stops_at_last_item() {
    let mut queue = queue(&["a", "b"], 0);

    assert_eq!(queue.advance(), Some(1));
    assert_eq!(queue.advance(), None);

    assert_eq!(current_id(&queue), "b");
}

#[test]
fn advance_with_repeat_all_wraps_to_first_item() {
    let mut queue = queue(&["a", "b"], 1);
    queue.set_repeat(RepeatMode::All);

    assert_eq!(queue.advance(), Some(0));

    assert_eq!(current_id(&queue), "a");
}

#[test]
fn advance_with_repeat_one_stays_on_current_item() {
    let mut queue = queue(&["a", "b"], 0);
    queue.set_repeat(RepeatMode::One);

    assert_eq!(queue.advance(), Some(0));

    assert_eq!(current_id(&queue), "a");
}

#[test]
fn advance_on_empty_queue_returns_none() {
    assert_eq!(Queue::new().advance(), None);
    assert_eq!(Queue::new().go_back(), None);
}

#[test]
fn go_back_stops_at_first_item_and_wraps_with_repeat_all() {
    let mut queue = queue(&["a", "b", "c"], 1);

    assert_eq!(queue.go_back(), Some(0));
    assert_eq!(queue.go_back(), None);
    queue.set_repeat(RepeatMode::All);
    assert_eq!(queue.go_back(), Some(2));

    assert_eq!(current_id(&queue), "c");
}

#[test]
fn jump_to_moves_by_user_index_in_shuffle_mode() {
    let mut queue = queue(&["a", "b", "c", "d"], 0);
    let _ = queue.set_shuffle(true, &mut rng());

    let position = queue.jump_to(2).expect("index 2 is valid");

    assert_eq!(current_id(&queue), "c");
    assert_eq!(queue.current_playback_index(), Some(position));
    assert_eq!(queue.jump_to(9), None);
}

#[test]
fn sync_to_playback_index_follows_the_engine() {
    let mut queue = queue(&["a", "b", "c", "d"], 0);
    let _ = queue.set_shuffle(true, &mut rng());
    let expected = playback(&queue)[2].clone();

    assert!(queue.sync_to_playback_index(2));
    assert!(!queue.sync_to_playback_index(4));

    assert_eq!(current_id(&queue), expected);
}

#[test]
fn history_records_left_items_and_keeps_100() {
    let ids: Vec<String> = (0..150).map(|number| number.to_string()).collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut queue = queue(&refs, 0);

    while queue.advance().is_some() {}

    let history: Vec<&str> = queue.history().map(ItemId::as_str).collect();
    assert_eq!(history.len(), 100);
    assert_eq!(history.first(), Some(&"49"));
    assert_eq!(history.last(), Some(&"148"));
}

#[test]
fn sync_to_current_position_does_not_touch_history() {
    let mut queue = queue(&["a", "b"], 0);

    assert!(queue.sync_to_playback_index(0));

    assert_eq!(queue.history().count(), 0);
}

#[test]
fn shuffle_accepts_the_thread_rng() {
    let mut queue = queue(&["a", "b", "c"], 0);

    let _ = queue.set_shuffle(true, &mut rand::rng());

    assert_eq!(playback(&queue)[0], "a");
}
