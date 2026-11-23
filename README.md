<p align="center">
  <img src="assets/ringomusic-banner.png" alt="RingoMusic: Apple Music for Linux, native GNOME style" width="640">
</p>

# RingoMusic

RingoMusic is an open-source client for Apple Music on Linux. It looks and feels like a native GNOME application. The user interface and all application logic are Rust code (GTK 4 and libadwaita). A hidden Chromium engine plays the DRM-protected audio.

Website: <https://ringomusic.netlify.app/>

RingoMusic is not affiliated with, endorsed by, or sponsored by Apple. You need an Apple Music subscription to use it.

**Status: Beta.**

<p align="center">
  <img src="assets/promo.png" alt="RingoMusic screenshot">
</p>

## Supported platforms

RingoMusic supports x86_64 Linux only. The minimum distributions are:

- Debian 13
- Ubuntu 26.04 LTS
- The two supported Fedora releases

These distributions have GTK 4.18, libadwaita 1.7, and glibc 2.41 or newer. Older distributions, such as Ubuntu 24.04, are not supported.

## Install

Download the files from the [releases page](https://github.com/MedaiP90/RingoMusic/releases).
Pick one format. RingoMusic is not in the distribution repositories.

| Format | Systems | Command |
|---|---|---|
| `.deb` | Debian 13, Ubuntu 26.04 | `sudo apt install ./ringomusic_<version>_amd64.deb` |
| `.rpm` | Fedora | `sudo dnf install ./ringomusic-<version>-1.x86_64.rpm` |
| Flatpak bundle | Any system with Flatpak | `flatpak install --user RingoMusic-<version>-x86_64.flatpak` |
| AppImage | Any system except Ubuntu | `chmod +x RingoMusic-<version>-x86_64.AppImage && ./RingoMusic-<version>-x86_64.AppImage` |
| AUR | Arch Linux | `yay -S ringomusic-bin` |
| Tarball | Other systems | See below |

Notes:

- **Flatpak:** the bundle adds no remote, so `flatpak update` does not update it. Install the new bundle to update.
- **AppImage:** the AppImage needs unprivileged user namespaces. Ubuntu blocks them, so it is not supported on Ubuntu (use the `.deb` or the Flatpak). To update, run `appimageupdatetool RingoMusic-<version>-x86_64.AppImage`. The application also shows a notice when a new version exists.
- **Tarball:** the archive has a `usr/` tree. Extract it into a prefix, for example `sudo tar -xzf ringomusic-<version>-x86_64.tar.gz -C /usr/local`. Then set the Chromium sandbox helper: `sudo chown root:root /usr/local/lib/ringomusic/engine/chrome-sandbox && sudo chmod 4755 /usr/local/lib/ringomusic/engine/chrome-sandbox`. Install GTK 4.18, libadwaita 1.7, NSS, and the other libraries of the `.deb` package yourself.
- If the player does not start, see [docs/troubleshooting.md](docs/troubleshooting.md).

### Verify the download

Each release has a `SHA256SUMS` file. Check your file:

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

The release files also have build provenance attestations. Verify a file with the GitHub CLI:

```sh
gh attestation verify <file> --repo MedaiP90/RingoMusic
```

## Architecture

```
┌──────────────────────── ringomusic (Rust) ────────────────────────┐
│  ui        gtk4 + libadwaita, relm4                               │
│     │ messages                                                    │
│  core      state store, queue model, cache (SQLite)               │
│     ├── am-api   typed Apple Music models over ApiTransport       │
│     ├── mpris    D-Bus MPRIS server                               │
│     └── engine   PlaybackEngine trait, sidecar supervisor         │
└───────────────────────────────┬───────────────────────────────────┘
                                │ JSON-RPC 2.0, NDJSON, Unix socket
┌───────────────────────────────▼───────────────────────────────────┐
│  ringomusic-engine (CastLabs Electron, hidden window)             │
│  music.apple.com → MusicKit.getInstance() → hook script           │
│  Widevine CDM → Chromium audio → PipeWire                         │
└───────────────────────────────────────────────────────────────────┘
```

Rust owns all state. The engine plays audio and sends Apple Music API requests. The engine keeps the login session. RingoMusic never reads or stores your credentials.

## Memory use

The player is a Chromium engine with several processes, so it uses more memory than a native player. Measured values (CastLabs Electron `v45.2.1+wvcus`, Manjaro, during playback):

| Measure | Engine processes (sum) |
|---|---|
| RSS | about 2590 MB |
| PSS | about 1265 MB |

RSS counts shared pages more than one time for each process, so PSS is the real cost. RSS stayed flat over 75 minutes (2588 to 2591 MB), so there is no sign of a leak. No 8-hour measurement exists, and the memory of the `ringomusic` process was not measured.

## Non-goals

- RingoMusic is not published on Flathub or in distribution repositories.
- RingoMusic does not download, export, or record audio. This rule is permanent.
- RingoMusic does not supply an equalizer or other audio processing in version 1.0.
- RingoMusic does not support aarch64. No official Widevine CDM exists for aarch64 Linux.
- RingoMusic does not supply lossless or Dolby Atmos audio. The web path gives AAC at 256 kbps only.

## Build from source

Requirements:

- Rust (stable toolchain)
- Node.js 24 or newer
- GTK 4.18 development files
- libadwaita 1.7 development files

Build the Rust workspace:

```sh
cargo build
```

Build the engine:

```sh
cd engine && npm ci && npm run build
```

Run the developer CLI against the engine. Build the engine first, then set the engine command:

```sh
export RINGO_ENGINE_CMD="$PWD/engine/node_modules/.bin/electron"
export RINGO_ENGINE_ARGS="$PWD/engine/"
cargo run -p ringo-cli -- signin
cargo run -p ringo-cli -- search radiohead
```

Run `ringo-cli --help` for all commands.

The GUI reads its settings from GSettings. Cargo compiles the schema into `target/`. Point
GSettings at it when you run the GUI from the source tree (`<profile>` is `debug` or `release`):

```sh
export GSETTINGS_SCHEMA_DIR="$(echo "$PWD"/target/<profile>/build/ringo-core-*/out/schemas)"
```

The build needs `glib-compile-schemas` (package `libglib2.0-bin` on Debian).

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl`+`Space` | Play or pause |
| `Ctrl`+`Right` | Next song |
| `Ctrl`+`Left` | Previous song |
| `Ctrl`+`S` | Toggle shuffle |
| `Ctrl`+`R` | Cycle repeat mode |
| `Ctrl`+`F` | Search |
| `F9` | Show or hide the queue |
| `F5` | Refresh the home page |
| `Ctrl`+`,` | Preferences |
| `Ctrl`+`Q` | Quit |
| `Esc` | Close a dialog or the queue panel, or go back |

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before you open a pull request.

## License

RingoMusic is free software. It uses the GNU General Public License, version 3 or later (`GPL-3.0-or-later`). See the [LICENSE](LICENSE) file.
