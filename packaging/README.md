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

## AppImage

`packaging/appimage/build-appimage.sh` builds
`dist/RingoMusic-<version>-x86_64.AppImage` and its `.zsync` file. Build on
Debian 13, because the AppImage uses the glibc of the build host and bundles
its libraries. Libraries of newer systems (Arch, Fedora) use RELR relocations,
and the `patchelf` of `linuxdeploy` corrupts them. The AppImage then crashes
at start, inside `ld.so`. On any other host, run
`packaging/appimage/build-in-container.sh`. It runs the build in a `debian:13`
container and needs Docker. The build needs network access.

`ringomusic-env.sh` is an AppRun hook. It unsets `GDK_BACKEND` and `GTK_THEME`,
which the GTK plugin forces.

1. The script runs `packaging/stage.sh /usr` (or reuses `stage/` when
   `RINGO_REUSE_STAGE=1`) and copies the tree to `AppDir/`.
2. It deletes `chrome-sandbox` from `AppDir/`.
3. `linuxdeploy` with the GTK plugin (`DEPLOY_GTK_VERSION=4`) bundles the
   libraries of `ringomusic` and of `ringomusic-engine`, GTK 4, and libadwaita.
   The plugin compiles the GSettings schemas and sets `GSETTINGS_SCHEMA_DIR` in
   its AppRun hook. The script fails if one of the two is missing.
4. `appimagetool` makes the AppImage with the static type2 runtime (no
   `libfuse2` is needed) and the update information
   `gh-releases-zsync|MedaiP90|RingoMusic|latest|RingoMusic-*-x86_64.AppImage.zsync`.
5. The script checks that `AppDir/` has no Widevine file and no
   `chrome-sandbox`, and validates the metainfo file.

The tools are pinned. `packaging/appimage/tools.sha256` has the SHA-256 of
each tool. When you change a version, change the URL in the script and the
checksum together.

Test in containers (no FUSE, so `APPIMAGE_EXTRACT_AND_RUN=1`):
`packaging/appimage/test-appimage.sh [IMAGE...]`.

### Why the AppImage has no `chrome-sandbox`

AppImage mounts are `nosuid`, so a SUID `chrome-sandbox` cannot work. A
helper without the SUID bit makes the engine fail. Without the helper,
Chromium uses the user namespace sandbox. This is the only sandbox mode of the
AppImage. `--no-sandbox` is still forbidden (decision D19).

### Not supported on Ubuntu

Ubuntu restricts unprivileged user namespaces with AppArmor. The `.deb`
package loads a profile for the engine, but an AppImage has no fixed path for
a profile. On Ubuntu, the AppImage shows the sandbox error page, with text
that tells the user to install the `.deb` package or the Flatpak.
