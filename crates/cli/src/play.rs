// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use anyhow::{Result, bail};
use ringo_engine::protocol::PlaybackState;
use ringo_engine::{EngineEvent, ItemId, PlaybackEngine};
use tokio::sync::broadcast::{self, error::RecvError};

/// Last known playback point. It restores playback after an engine restart.
///
/// ponytail: spike restore. Prompt 21 replaces it with the persisted queue.
#[derive(Debug)]
struct ResumePoint {
    queue: Vec<ItemId>,
    index: usize,
    position: Duration,
}

impl ResumePoint {
    fn new(queue: Vec<ItemId>) -> Self {
        Self {
            queue,
            index: 0,
            position: Duration::ZERO,
        }
    }

    /// Remembers the index and the position from the player events.
    fn observe(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::NowPlaying { queue_index, .. } => {
                self.index = *queue_index;
                self.position = Duration::ZERO;
            }
            EngineEvent::Position { position } => self.position = *position,
            _ => {}
        }
    }
}

/// Plays the songs and prints the player events until the queue ends.
///
/// After an engine restart, it sets the queue again, seeks, and plays.
pub async fn play(
    engine: &impl PlaybackEngine,
    song_ids: Vec<String>,
    mut events: broadcast::Receiver<EngineEvent>,
) -> Result<()> {
    let queue: Vec<ItemId> = song_ids.into_iter().map(ItemId::new).collect();
    let mut resume = ResumePoint::new(queue.clone());
    engine.set_queue(queue, 0).await?;
    engine.play().await?;

    loop {
        let event = match events.recv().await {
            Ok(event) => event,
            Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => bail!("engine stopped"),
        };
        resume.observe(&event);
        print_event(&event);
        match event {
            EngineEvent::State {
                state: PlaybackState::Ended,
            } => return Ok(()),
            EngineEvent::Restarted => restore(engine, &resume).await?,
            EngineEvent::GaveUp => bail!("engine did not restart"),
            _ => {}
        }
    }
}

/// Sends the queue again from the last index, seeks to the last position, and plays.
async fn restore(engine: &impl PlaybackEngine, resume: &ResumePoint) -> Result<()> {
    engine.set_queue(resume.queue.clone(), resume.index).await?;
    engine.seek(resume.position).await?;
    engine.play().await?;
    Ok(())
}

/// Prints the player events, and the events that explain a change in playback.
fn print_event(event: &EngineEvent) {
    match event {
        EngineEvent::State { state } => println!("state: {state:?}"),
        EngineEvent::NowPlaying {
            title,
            artist,
            album,
            queue_index,
            ..
        } => println!("now playing #{queue_index}: {title} - {artist} ({album})"),
        EngineEvent::Position { position } => println!("position: {}s", position.as_secs()),
        EngineEvent::Error { code, message, .. } => println!("error {code:?}: {message}"),
        EngineEvent::Restarted => println!("engine restarted"),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now_playing(queue_index: usize) -> EngineEvent {
        EngineEvent::NowPlaying {
            id: ItemId::new("1"),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            duration: Duration::from_secs(200),
            artwork_url: None,
            queue_index,
        }
    }

    #[test]
    fn observe_keeps_the_last_index_and_position() {
        let mut resume = ResumePoint::new(vec![ItemId::new("1"), ItemId::new("2")]);
        resume.observe(&now_playing(1));
        resume.observe(&EngineEvent::Position {
            position: Duration::from_secs(42),
        });
        assert_eq!(resume.index, 1);
        assert_eq!(resume.position, Duration::from_secs(42));
    }

    #[test]
    fn a_new_item_resets_the_position() {
        let mut resume = ResumePoint::new(vec![ItemId::new("1")]);
        resume.observe(&EngineEvent::Position {
            position: Duration::from_secs(42),
        });
        resume.observe(&now_playing(0));
        assert_eq!(resume.position, Duration::ZERO);
    }
}
