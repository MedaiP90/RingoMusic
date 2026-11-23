<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 RingoMusic contributors
-->

# Troubleshooting

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
supported on Ubuntu. Use the `.deb` package or the Flatpak.

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

## Do not use `--no-sandbox`

Do not start the engine with `--no-sandbox`. RingoMusic does not support it,
and it removes an important security protection. Fix the installation instead.

## Report a problem

If the steps do not help, open an issue at
<https://github.com/MedaiP90/RingoMusic/issues>. Include:

- The package format and the version of RingoMusic (`ringomusic --version`).
- Your distribution and its version.
- The output of `ringomusic` started from a terminal.
