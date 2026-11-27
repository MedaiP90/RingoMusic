# Contributing to RingoMusic

Thank you for your interest in RingoMusic. This file gives the rules for all contributions.

## Project rules

These rules apply to all contributions.

1. Do not add code that downloads, decrypts, or saves audio streams.
2. Each request that changes the user's library starts from one direct user action. Do not add automatic, scheduled, background, or bulk writes. Do not add automatic retries of a failed write. Do not send writes from the CLI, MPRIS, D-Bus, or a plug-in or scripting interface. These rules have no exceptions, also for optional features.
3. Use only write endpoints that Apple documents in the [Apple Music API](https://developer.apple.com/documentation/applemusicapi). Add a link to the documentation page in the doc comment of each write function. Add each new write endpoint to `WRITE_PATTERNS` in `crates/engine/src/protocol/to_engine.rs` and in `engine/src/api-write.ts`: the app refuses each other write request.
4. Do not put the Widevine CDM in the repository or in a package. The engine downloads the CDM at runtime.
5. Do not use Apple logos, the Apple Music icon, or the Apple Music name as part of the product name or icon.
6. All code, comments, and commit messages are in English.
7. All user-facing text and documentation follow Simplified Technical English principles.

## Application ID

The application ID is `io.github.MedaiP90.RingoMusic`. Do not change it.

With a changed ID, Flatpak and the desktop see a new application. User settings and the login session do not move to the new ID.

## Commit messages

Use this format:

```
<module>: <Imperative summary>

<Body>
```

- The first line has 72 characters or fewer. It has no period at the end.
- Start the summary with a verb in the imperative mood.
- Put a blank line after the first line.
- In the body, explain what changed and why. Do not explain how. Use lines of 75 characters or fewer.

Examples:

```
engine: Add socket supervisor

Restart the engine when the process exits. Restore the queue and the
playback position after the restart.
```

```
docs: Add README and contribution guide

Describe the project, its limits, and how to build it. Record the
project rules, the commit style, and the license header rules.
```

## License headers

RingoMusic uses `GPL-3.0-or-later`. Put this header at the start of each source file:

```
// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) <year> RingoMusic contributors
```

Rules for reused code:

<!-- REUSE-IgnoreStart -->
- You can reuse code from AGPL-3.0 projects. The reused parts stay under AGPL-3.0. Mark these files with `SPDX-License-Identifier: AGPL-3.0-or-later` and the original copyright.
<!-- REUSE-IgnoreEnd -->
- Before you reuse code from any other project, check that its license is compatible with GPL-3.0.

## Checks before a pull request

Run these checks. All of them must pass.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
(cd engine && npm run check)
(cd engine && npm run gen:protocol) && git diff --exit-code protocol/ engine/src/protocol.ts
reuse lint
```

The last two checks apply when the project has the protocol generator and the `reuse` configuration.
