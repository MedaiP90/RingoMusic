<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 RingoMusic contributors
-->

# Engine protocol

This file describes the behavior of the protocol between Rust and the engine. The Rust types in `crates/engine/src/protocol/` are the source of truth for the shape of each message. `protocol/schema.json` and `engine/src/protocol.ts` are generated from them. `protocol/examples/` has one example for each method and notification.

## Regenerate the files

```sh
RINGO_UPDATE_SCHEMA=1 cargo test -p ringo-engine schema_is_up_to_date
(cd engine && npm run gen:protocol)
```

CI runs both commands and fails if `git diff` shows a change.

## Transport

1. Rust creates a Unix socket in `$XDG_RUNTIME_DIR/ringomusic/engine.sock`. The directory mode is `0700`.
2. Rust starts the engine and gives the socket path in `RINGO_ENGINE_SOCKET`.
3. The engine connects to the socket.
4. Each message is one JSON-RPC 2.0 object on one line (NDJSON).

The protocol does not use stdio. Chromium writes to stdout and stderr.

**The engine quits when the socket closes.** There is no reconnect. Rust starts a new engine process if it needs one.

## Handshake

The engine sends `engine.hello` first. Rust does not send a request before it gets this message.

```json
{"jsonrpc":"2.0","method":"engine.hello","params":{"proto":1,"engine_version":"0.1.0","electron":"41.0.0"}}
```

If `proto` is not `PROTOCOL_VERSION` (currently `1`), Rust stops the engine and shows an error. Any change that breaks compatibility increases the version.

After the handshake, the engine sends `engine.ready` when Widevine is ready and MusicKit is available. Before `engine.ready`, methods other than `engine.shutdown` can fail with `not_ready`.

## Requests and responses

A request has `jsonrpc`, `id` (an unsigned integer, unique in the connection), `method`, and `params` (omitted if the method has none). The engine sends exactly one response for each request, with the same `id`:

- Success: `{ "jsonrpc": "2.0", "id": 1, "result": ... }`. A method without a result returns `null`.
- Error: `{ "jsonrpc": "2.0", "id": 1, "error": { "code": -32001, "message": "..." } }`. The `id` is `null` if the engine cannot read the request.

A notification has no `id` and gets no response.

## Methods (Rust → engine)

| Method | Params | Result |
|---|---|---|
| `auth.status` | none | `{ authorized, storefront }` |
| `auth.signIn` | none | `null`. Loads `https://music.apple.com/login`, which opens the site's own sign-in dialog, then shows the window. If the load fails, the window shows anyway and the user can click the site's Sign In button. Returns when `auth.changed` reports `authorized: true`, or fails with `cancelled` if the user closes the window. Returns at once if already authorized. |
| `auth.signOut` | none | `null`. Drops the MusicKit token and clears the cookies of the session. The page would sign in again from the Apple ID cookies without the clear. Other storage stays, because it holds the device identity. |
| `player.setQueue` | `{ items, start_index, kind }` | `null`. `kind` is `songs`. IDs can be catalog IDs or library IDs (`i.…`). |
| `player.playNext`, `player.playLater` | `{ items, kind }` | `null`. Insert after the current item, or at the end. The current item keeps playing. |
| `player.remove` | `{ index }` | `null`. Removes the item at `index` of the engine queue. Rust never removes the current item. |
| `player.play`, `player.pause`, `player.stop` | none | `null` |
| `player.next`, `player.previous` | none | `null` |
| `player.seek` | `{ position_ms }` | `null` |
| `player.setVolume` | `{ volume }` (0.0 to 1.0) | `null` |
| `player.setRepeat` | `{ mode }` (`none`, `one`, `all`) | `null` |
| `api.get` | `{ path, query }` | Raw JSON from `api.music()` |
| `engine.shutdown` | none | `null`. The engine answers, then quits. |

**Sign-in note:** MusicKit `authorize()` does not work on music.apple.com. The site resets the token from its own storage right after the pop-up closes (status 3, then 0). Sign-in through the site's own button stays authorized. Checked with a real account. The `/login` route opens the same dialog as the button. Checked with a real account: the login stays after a restart, and a sign-in after a sign-out opens the dialog again.

**Unverified:** library song IDs (`i.…`) in `setQueue({ songs })` need a test with a real account. Result: pending.

Rust owns the queue order, including shuffle. The engine never shuffles. `player.setQueue` always gets the full ordered list. During playback, Rust changes only the upcoming items with `player.playNext`, `player.playLater`, and `player.remove`, so the current song does not stop. These methods use `queue.remove`, `playNext`, and `playLater` of MusicKit.

## Notifications (engine → Rust)

| Method | Params |
|---|---|
| `engine.hello` | `{ proto, engine_version, electron }` |
| `engine.ready` | none |
| `engine.incompatible` | `{ check, detail }`. A self-check failed. |
| `engine.log` | `{ level, message, fields }` |
| `auth.changed` | `{ authorized }` |
| `player.state` | `{ state }`: `playing`, `paused`, `stopped`, `loading`, or `ended` |
| `player.nowPlaying` | `{ id, title, artist, album, duration_ms, artwork_url, album_id, queue_index }`. `album_id` is `null` when MusicKit has no album link for the item. `queue_index` is the index in the list from `player.setQueue`. |
| `player.position` | `{ position_ms }` |
| `error` | `{ code, message, fatal }`. If `fatal` is `true`, the engine quits after it. |

### `player.position` rate limit

The engine sends at most 4 `player.position` notifications each second.

### `engine.log` rules

- `level` is `trace`, `debug`, `info`, `warn`, or `error`.
- Rust re-emits each record with `tracing`, target `engine`.
- The engine does not put secrets in `message` or `fields`: no tokens, cookies, or user IDs.
- Before the socket connects, the engine logs to stderr. After it connects, the engine uses `engine.log`.

## Error codes

Codes are 32-bit integers. Application codes are in the range -32000 to -32099.

| Code | Name | Meaning |
|---|---|---|
| -32700 | `parse_error` | The message is not valid JSON. |
| -32600 | `invalid_request` | The message is not a valid JSON-RPC object. |
| -32601 | `method_not_found` | The method does not exist. |
| -32602 | `invalid_params` | The params are not valid for the method. |
| -32603 | `internal_error` | The engine had an internal fault. |
| -32001 | `not_authorized` | The user did not authorize the app. |
| -32002 | `no_subscription` | The account has no Apple Music subscription. |
| -32003 | `not_ready` | Widevine or MusicKit is not ready. |
| -32004 | `cancelled` | The user cancelled the operation. |
| -32005 | `musickit_error` | MusicKit returned an error. |
| -32006 | `sandbox_failed` | The Chromium sandbox cannot start. The supervisor sets this code. The engine does not send it. |
