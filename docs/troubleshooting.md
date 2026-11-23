<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 RingoMusic contributors
-->

# Troubleshooting

Find your problem in this list:

- [No sound](#no-sound)
- [Login loop](#login-loop)
- ["Engine incompatible"](#engine-incompatible)
- [Chromium sandbox errors](#chromium-sandbox)
- [Flatpak: the music stops when the window closes](#flatpak-background-permission)
- [How to collect logs](#collect-logs)

## No sound

1. Check that another application plays sound through the same output device.
2. Start a song in RingoMusic. Open the sound settings of your system, or run
   `pw-top` or `pactl list sink-inputs`. A stream named `ringomusic-engine`
   must be there. If the stream is "corked" or has no volume, raise its
   volume in the sound settings.
3. If there is no stream, the player did not start playing. Press **Play**
   again. If nothing happens, collect the logs (see [How to collect
   logs](#collect-logs)) and look for the word `error`.
4. Flatpak: the application needs access to the sound server. Run
   `flatpak info --show-permissions io.github.MedaiP90.RingoMusic`. The list
   must have `sockets=...;pulseaudio;...`. Do not remove this permission.
5. Some songs are not available in your country. The player shows an error for
   these songs and goes to the next song.
6. Your Apple Music subscription must be active. Open
   <https://music.apple.com> in a browser and check that you can play a song.

## Login loop

The login page opens again and again, or the application says that you are
signed out after each start.

1. Choose **Sign Out** in the preferences. Then choose **Sign In** again.
2. Check that the clock of your system is correct. A wrong time makes the
   Apple login fail.
3. Check that the disk is not full and that you can write to the data
   directory of the player:
   - Native packages and AppImage: `~/.local/share/ringomusic/engine`
   - Flatpak: `~/.var/app/io.github.MedaiP90.RingoMusic/data/ringomusic/engine`
4. Close RingoMusic. Move the `engine` directory away (do not delete it yet).
   Start RingoMusic and sign in again. This makes a clean session. If the
   login works, you can delete the old directory.

Each new login can register a new device in your Apple account. Apple limits
the number of devices. Do not repeat a clean login many times.

## Engine incompatible

The page "The player does not work on this system" shows the name of a failed
check and a detail. The player checks that the Apple Music web player still has
the parts that RingoMusic needs. This happens when:

- Apple changed its web player. The application must be updated. Check for a
  new release at <https://github.com/MedaiP90/RingoMusic/releases>.
- The network blocks `music.apple.com` or one of its scripts. Check your
  firewall, VPN, DNS filter, or ad blocker at network level. The check
  `musickit.instance` means that the page loaded but MusicKit did not start in
  30 seconds.

If a new release does not fix it, open an issue and include the check name and
the detail text.

## Chromium sandbox

RingoMusic plays music with a separate player process (the engine). The
engine is based on Chromium, which runs inside a security sandbox. RingoMusic
never starts the engine without the sandbox. If the sandbox cannot start, the
app shows the page "The player cannot start".

Find your package format below. The format decides the fix.

### `.deb` or `.rpm` package

The package installs a helper, `chrome-sandbox`, that must have the owner
`root` and the mode `4755`. Check it:

```sh
stat -c '%a %U' /usr/lib/ringomusic/engine/chrome-sandbox
```

The result must be `4755 root`. If it is not, install the package again:

```sh
sudo apt install --reinstall ringomusic     # Debian, Ubuntu
sudo dnf reinstall ringomusic               # Fedora
```

Some file systems mount `/usr` with the option `nosuid`. The helper cannot
work there. Mount `/usr` without `nosuid`, or enable unprivileged user
namespaces (see below).

### Ubuntu 24.04 and later

Ubuntu restricts unprivileged user namespaces with AppArmor. The `.deb`
package installs the profile `/etc/apparmor.d/ringomusic` that allows the
engine. If the app still shows the error, check that the profile is loaded:

```sh
sudo apparmor_parser -r -T -W /etc/apparmor.d/ringomusic
```

Then choose **Try Again** in the app.

### AppImage

The AppImage has no `chrome-sandbox`, because AppImage mounts are `nosuid`.
The engine uses unprivileged user namespaces instead. Check that your system
allows them:

```sh
sysctl kernel.unprivileged_userns_clone     # Debian-based kernels
sysctl user.max_user_namespaces             # must be more than 0
```

On **Ubuntu**, AppArmor blocks them for the AppImage. The AppImage is not
supported on Ubuntu, and the error "The player cannot start" is expected
there. The AppImage has no fixed path, so no AppArmor profile can allow it.
Use the `.deb` package or the Flatpak.

On other systems, a hardened kernel or a container can also block user
namespaces. Enable them (for example `sudo sysctl -w user.max_user_namespaces=15000`)
or use another package format.

### Flatpak

The Flatpak starts the engine with `zypak`, which uses the sandbox of
Flatpak. You do not need to change the system. Update the runtime and the
app:

```sh
flatpak update
```

If the error stays, report it (see below).

### Arch Linux (AUR)

The `PKGBUILD` installs `chrome-sandbox` with the mode `4755`. If the system
mounts `/usr` with `nosuid`, see the section on `.deb` and `.rpm` packages.

## Flatpak background permission

When you close the window, RingoMusic can keep playing in the background. The
Flatpak version must ask the system for this permission. The first time you
close the window, the system shows a question. If you answered **No**, the
music stops when you close the window.

To allow it again:

1. Open the system settings, then **Apps**, then **RingoMusic**, and allow
   **Background Activity**.
2. Or run in a terminal:

   ```sh
   flatpak permission-reset io.github.MedaiP90.RingoMusic
   ```

   Then close the window again and answer **Allow**.

You can also turn on **Quit when the window is closed** in the preferences. The
application then always stops with the window.

## Collect logs

Start RingoMusic from a terminal with more log output:

```sh
RUST_LOG=debug ringomusic 2>&1 | tee ringomusic.log
```

For the Flatpak:

```sh
flatpak run --env=RUST_LOG=debug io.github.MedaiP90.RingoMusic 2>&1 | tee ringomusic.log
```

For the AppImage, start the `.AppImage` file in the same way.

The log includes the messages of the player (`target: engine`). To see more
detail from the player itself, also set `LOG_LEVEL=debug`.

Before you share a log, read it. It contains song names and file paths. It
does not contain your password or your Apple tokens.

## Do not use `--no-sandbox`

Do not start the engine with `--no-sandbox`. RingoMusic does not support it,
and it removes an important security protection. Fix the installation instead.

## Report a problem

If the steps do not help, open an issue at
<https://github.com/MedaiP90/RingoMusic/issues>. Include:

- The package format and the version of RingoMusic (`ringomusic --version`).
- Your distribution and its version.
- The log from [How to collect logs](#collect-logs).
