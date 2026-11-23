<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 RingoMusic contributors
-->

# Updating CastLabs Electron

The engine runs on CastLabs Electron (`+wvcus` builds), because only these
builds include Widevine. Update it only to a tested CastLabs release. Each
update must pass the Phase 1 acceptance checklist.

## When to update

The workflow `.github/workflows/castlabs-check.yml` runs each week. When
CastLabs publishes a new release, it opens an issue with the label
`castlabs-update`. Update when the issue brings a Chromium security fix, or
when the new release has a change that you need.

Use only stable `+wvcus` tags. Do not use alpha or beta builds.

## Steps

1. Choose the new tag from <https://github.com/castlabs/electron-releases/releases>.
2. Change the pinned version in two places. Use the same tag in both:
   - `engine/package.json`: the `electron` entry
     (`github:castlabs/electron-releases#<tag>`).
   - `engine/CASTLABS_VERSION.md`: the `Tag`, the pin date, and the note.
3. Update the lock file:

   ```sh
   cd engine && npm install && npm run check && cd ..
   ```

4. Update the checksums. The script downloads the archive, writes
   `packaging/castlabs.sha256`, and changes the URL and the SHA-256 in the
   Flatpak manifest:

   ```sh
   packaging/update-castlabs-sha.sh
   ```

5. Check the user agent. The engine builds it from the Chrome major version of
   the new Electron (`buildUserAgent` in `engine/src/main.ts`). Start the
   engine with `LOG_LEVEL=info` and read the `page identity` log line. The
   user agent must show the new Chrome version, and the brands must not show
   "Electron". If the Apple Music page rejects the browser, check this first.
6. Run the Phase 1 acceptance checklist
   (`plan/01_first_implementation/13-phase1-acceptance.md`, steps 1.1 to 1.8).
   Record the results in a new section of
   `plan/01_first_implementation/results/`.
7. Run all checks:

   ```sh
   cargo fmt --all --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --all-features
   (cd engine && npm run check)
   ```

8. Build one package (for example `packaging/build-packages.sh`) and start it.
   Play a song.
9. Commit the changes with the message `engine: Update CastLabs Electron to <tag>`.
   In the pull request, write `Closes #<number>` for the `castlabs-update` issue.
   The issue closes when the pull request is merged. If you do not update, close
   the issue with a short reason.

## If the update fails

- Widevine does not load: check the CastLabs release notes for a new CDM
  requirement.
- A check of the engine fails ("engine incompatible"): see
  [troubleshooting](troubleshooting.md#engine-incompatible).
- The sandbox fails: see [troubleshooting](troubleshooting.md#chromium-sandbox).
- Go back to the old tag. Do not release an untested version.
