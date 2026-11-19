// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::time::Duration;

use tokio::sync::broadcast;

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

    /// Stops the engine and waits until it quits. No restart follows.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine task failed.
    async fn shutdown(&self) -> Result<()>;

    /// Subscribes to engine events.
    fn events(&self) -> broadcast::Receiver<EngineEvent>;
}
