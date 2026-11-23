#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Builds the Flatpak bundle dist/RingoMusic-<version>-x86_64.flatpak.
# Needs flatpak, flatpak-builder, and the GNOME 51 runtime and SDK from Flathub.
set -euo pipefail

# shellcheck source=packaging/castlabs-lib.sh
source "$(dirname "$0")/castlabs-lib.sh"

APP_ID="io.github.MedaiP90.RingoMusic"
MANIFEST="$PACKAGING_DIR/flatpak/$APP_ID.yml"
RUNTIME_REPO="https://dl.flathub.org/repo/flathub.flatpakrepo"

cd "$REPO_ROOT"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
bundle="dist/RingoMusic-$version-x86_64.flatpak"

# The manifest pins the same archive as packaging/castlabs.sha256.
expected="$(cut -d' ' -f1 "$CASTLABS_SHA_FILE")"
if ! grep -q "sha256: $expected" "$MANIFEST"; then
    echo "error: $MANIFEST does not use the SHA-256 of packaging/castlabs.sha256" >&2
    exit 1
fi

flatpak-builder --force-clean --user --install-deps-from=flathub \
    --repo=repo builddir "$MANIFEST"

# The metainfo file must be valid in the install tree.
appstreamcli validate --no-net "builddir/files/share/metainfo/$APP_ID.metainfo.xml"

# The app stops at start without the compiled GSettings schemas.
if [ ! -f builddir/files/share/glib-2.0/schemas/gschemas.compiled ]; then
    echo "error: no compiled GSettings schemas in the Flatpak" >&2
    exit 1
fi

# The bundle must not contain Widevine (the CDM is downloaded at run time).
if find builddir/files -iname '*widevine*' | grep -q .; then
    echo "error: Widevine files in the Flatpak" >&2
    exit 1
fi

mkdir -p dist
flatpak build-bundle repo "$bundle" "$APP_ID" --runtime-repo="$RUNTIME_REPO"
echo "bundle: $bundle"
