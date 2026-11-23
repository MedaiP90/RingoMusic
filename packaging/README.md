<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 RingoMusic contributors
-->

# Packaging notes

## Chromium sandbox

The engine is Electron, which uses the Chromium sandbox. The sandbox is
required on all native targets.

### Why `chrome-sandbox` is SUID root

`chrome-sandbox` is the helper that sets up the sandbox when the kernel does
not allow unprivileged user namespaces. It must have owner `root` and mode
`4755`. If the owner or the mode is wrong, Chromium stops with an error that
names `chrome-sandbox`.

### Why the AppArmor profile exists

Ubuntu 24.04 and later restrict unprivileged user namespaces with AppArmor.
Without a profile, the engine cannot create the namespace and the sandbox
fails. `packaging/apparmor/ringomusic` gives the engine binary the `userns`
permission and nothing else (`flags=(unconfined)`). The package scripts load
the profile. Check the syntax with:

```sh
apparmor_parser -Q -T packaging/apparmor/ringomusic
```

### `--no-sandbox` is forbidden

Never start the engine with `--no-sandbox`, also not in development (decision
D19). If the sandbox fails, fix the installation. When the engine exits
at start with a sandbox error, the supervisor reports
`ErrorCode::SandboxFailed`, does not restart the engine, and the app shows a
page with a link to the troubleshooting guide.

## Flatpak bundle

`packaging/build-flatpak.sh` builds `dist/RingoMusic-<version>-x86_64.flatpak`.
It needs `flatpak-builder`. The build needs network access (Cargo, npm).

- Runtime: GNOME 51, which uses freedesktop 26.08. The Electron base app
  (`26.08`) and the SDK extensions `rust-stable` and `node24` use the same
  branch. When you change the runtime, change all four together.
- The manifest runs `packaging/stage.sh /app`. The CastLabs archive is a manifest
  source. `stage.sh` reads its path from `RINGO_CASTLABS_ZIP`. Keep the SHA-256
  in the manifest equal to `packaging/castlabs.sha256`. `update-castlabs-sha.sh`
  updates both. The build script checks them.
- Inside Flatpak, the application starts the engine with `zypak-wrapper`.
  `chrome-sandbox` has no SUID bit there.
- Install: `flatpak install --user dist/RingoMusic-<version>-x86_64.flatpak`.
