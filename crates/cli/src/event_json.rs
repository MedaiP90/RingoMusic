// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use ringo_engine::EngineEvent;
use serde_json::{Value, json};

/// Converts an engine event to JSON. The event name is in the `event` field.
pub fn to_json(event: &EngineEvent) -> Value {
    match event {
        EngineEvent::Ready => json!({ "event": "ready" }),
        EngineEvent::Incompatible { check, detail } => {
            json!({ "event": "incompatible", "check": check, "detail": detail })
        }
        EngineEvent::ProtocolMismatch { expected, actual } => {
            json!({ "event": "protocol.mismatch", "expected": expected, "actual": actual })
        }
        EngineEvent::AuthChanged { authorized } => {
            json!({ "event": "auth.changed", "authorized": authorized })
        }
        EngineEvent::State { state } => json!({ "event": "player.state", "state": state }),
        EngineEvent::NowPlaying {
            id,
            title,
            artist,
            album,
            duration,
            artwork_url,
            album_id,
            queue_index,
        } => json!({
            "event": "player.nowPlaying",
            "id": id.as_str(),
            "title": title,
            "artist": artist,
            "album": album,
            "duration_ms": duration.as_millis(),
            "artwork_url": artwork_url,
            "album_id": album_id,
            "queue_index": queue_index,
        }),
        EngineEvent::Position { position } => {
            json!({ "event": "player.position", "position_ms": position.as_millis() })
        }
        EngineEvent::Error {
            code,
            message,
            fatal,
        } => json!({ "event": "error", "code": code, "message": message, "fatal": fatal }),
        EngineEvent::Restarted => json!({ "event": "restarted" }),
        EngineEvent::GaveUp => json!({ "event": "gave_up" }),
        // The enum is non-exhaustive. New events show up in the debug form.
        other => json!({ "event": "unknown", "debug": format!("{other:?}") }),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn position_event_uses_milliseconds() {
        let event = EngineEvent::Position {
            position: Duration::from_millis(1500),
        };
        assert_eq!(
            to_json(&event),
            json!({ "event": "player.position", "position_ms": 1500 })
        );
    }
}
