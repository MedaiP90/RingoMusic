// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use tokio::sync::{broadcast, watch};

use crate::protocol::{AuthStatus, RepeatMode};
use crate::{EngineEvent, ItemId, Result};

/// Playback engine that the rest of the app uses.
///
/// The engine keeps no queue state for the caller. After
/// [`EngineEvent::Restarted`], the caller sets the queue again.
#[async_trait::async_trait]
pub trait PlaybackEngine: Send + Sync {
    /// Returns the authorization state and the storefront.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn auth_status(&self) -> Result<AuthStatus>;

    /// Shows the login window. Returns when the login completes or the user cancels.
    /// This call has no time limit.
    ///
    /// # Errors
    ///
    /// Returns an error if the user cancels or the engine fails.
    async fn sign_in(&self) -> Result<()>;

    /// Signs the user out.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn sign_out(&self) -> Result<()>;

    /// Replaces the queue. `items` is in play order. `start` is the index of the first item.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn set_queue(&self, items: Vec<ItemId>, start: usize) -> Result<()>;

    /// Inserts items after the current item. The current item keeps playing.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn play_next(&self, items: Vec<ItemId>) -> Result<()>;

    /// Adds items at the end of the queue. The current item keeps playing.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer, or if
    /// the engine cannot add the items.
    async fn play_later(&self, items: Vec<ItemId>) -> Result<()>;

    /// Removes the item at `index` from the queue. The current item cannot be removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready, does not answer, or
    /// cannot remove the item.
    async fn remove(&self, index: usize) -> Result<()>;

    /// Starts or resumes playback.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn play(&self) -> Result<()>;

    /// Pauses playback.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn pause(&self) -> Result<()>;

    /// Stops playback.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn stop(&self) -> Result<()>;

    /// Plays the next queue item.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn next(&self) -> Result<()>;

    /// Plays the previous queue item.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn previous(&self) -> Result<()>;

    /// Seeks in the current item.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn seek(&self, position: Duration) -> Result<()>;

    /// Sets the volume. `volume` is from 0.0 to 1.0.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn set_volume(&self, volume: f32) -> Result<()>;

    /// Sets the repeat mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not ready or does not answer.
    async fn set_repeat(&self, mode: RepeatMode) -> Result<()>;

    /// Starts the engine again after [`EngineEvent::GaveUp`]. The restart limit
    /// begins again. The call does nothing while the supervisor still restarts
    /// the engine by itself.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine was stopped by [`PlaybackEngine::shutdown`].
    async fn restart(&self) -> Result<()>;

    /// Stops the engine and waits until it quits. No restart follows.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine task failed.
    async fn shutdown(&self) -> Result<()>;

    /// Subscribes to engine events.
    fn events(&self) -> broadcast::Receiver<EngineEvent>;

    /// Returns the last lifecycle event (see [`EngineEvent::is_lifecycle`]), or
    /// `None` before the first one. A late subscriber reads it to learn the
    /// current engine status.
    fn lifecycle(&self) -> watch::Receiver<Option<EngineEvent>>;
}
