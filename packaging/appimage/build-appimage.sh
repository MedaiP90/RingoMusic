#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Builds dist/RingoMusic-<version>-x86_64.AppImage and its .zsync file.
# Build on Debian 13 or newer: the AppImage uses the glibc of the host.
# Usage: packaging/appimage/build-appimage.sh
# Set RINGO_REUSE_STAGE=1 to reuse an existing stage/ built for the prefix /usr.
set -euo pipefail

# shellcheck source=packaging/castlabs-lib.sh
source "$(dirname "$0")/../castlabs-lib.sh"

APP_ID="io.github.MedaiP90.RingoMusic"
UPDATE_INFO="gh-releases-zsync|MedaiP90|RingoMusic|latest|RingoMusic-*-x86_64.AppImage.zsync"

# Pinned tools. The SHA-256 of each file is in tools.sha256.
LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage"
GTK_PLUGIN_URL="https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/7a3fbc31a9e5075073ff8790f26effbac5f84453/linuxdeploy-plugin-gtk.sh"
APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
RUNTIME_URL="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"

SHA_FILE="$PACKAGING_DIR/appimage/tools.sha256"
TOOLS_DIR="${RINGO_BUILD_CACHE:-$HOME/.cache/ringomusic-build}/appimage-tools"
APPDIR="$REPO_ROOT/AppDir"

# Downloads $1 as $2 in the tools directory and checks its SHA-256.
fetch_tool() {
    local url="$1" name="$2" expected actual
    mkdir -p "$TOOLS_DIR"
    if [ ! -f "$TOOLS_DIR/$name" ]; then
        echo "downloading $url" >&2
        curl --fail --location --retry 3 --silent --show-error \
            --output "$TOOLS_DIR/$name.part" "$url"
        mv "$TOOLS_DIR/$name.part" "$TOOLS_DIR/$name"
    fi
    expected="$(awk -v n="$name" '$2 == n { print $1 }' "$SHA_FILE")"
    actual="$(sha256sum "$TOOLS_DIR/$name" | cut -d' ' -f1)"
    if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
        rm -f "$TOOLS_DIR/$name"
        echo "error: SHA-256 of $name does not match $SHA_FILE" >&2
        echo "  expected ${expected:-<missing>}" >&2
        echo "  actual   $actual" >&2
        exit 1
    fi
    chmod +x "$TOOLS_DIR/$name"
}

fetch_all_tools() {
    fetch_tool "$LINUXDEPLOY_URL" linuxdeploy-x86_64.AppImage
    fetch_tool "$GTK_PLUGIN_URL" linuxdeploy-plugin-gtk.sh
    fetch_tool "$APPIMAGETOOL_URL" appimagetool-x86_64.AppImage
    fetch_tool "$RUNTIME_URL" runtime-x86_64
}

# Creates AppDir/ from the install tree. Without chrome-sandbox, Chromium uses
# the user namespace sandbox. AppImage mounts are nosuid, so a SUID helper
# would not work (decision D25). Never add --no-sandbox (decision D19).
prepare_appdir() {
    if [ "${RINGO_REUSE_STAGE:-0}" != 1 ] || [ ! -d "$REPO_ROOT/stage/usr" ]; then
        "$PACKAGING_DIR/stage.sh" /usr
    fi
    rm -rf "$APPDIR"
    cp -a "$REPO_ROOT/stage/." "$APPDIR/"
    rm -f "$APPDIR/usr/lib/ringomusic/engine/chrome-sandbox"
    # linuxdeploy sources the hooks of this directory in AppRun. The name sorts after the GTK plugin hook.
    install -Dm644 "$PACKAGING_DIR/appimage/ringomusic-env.sh" "$APPDIR/apprun-hooks/ringomusic-env.sh"
}

# Bundles the libraries of both executables, GTK 4, and libadwaita.
# linuxdeploy keeps its default exclude list (glibc, GPU driver libraries).
run_linuxdeploy() {
    local engine="usr/lib/ringomusic/engine/ringomusic-engine"
    (
        cd "$REPO_ROOT"
        export PATH="$TOOLS_DIR:$PATH"
        export APPIMAGE_EXTRACT_AND_RUN=1
        export DEPLOY_GTK_VERSION=4
        export LDAI_NO_APPSTREAM=1
        # The strip of linuxdeploy cannot read .relr.dyn sections of new libraries.
        export NO_STRIP=1
        "$TOOLS_DIR/linuxdeploy-x86_64.AppImage" \
            --appdir "$APPDIR" \
            --desktop-file "data/$APP_ID.desktop" \
            --icon-file "data/icons/$APP_ID.svg" \
            --executable "$APPDIR/usr/bin/ringomusic" \
            --executable "$APPDIR/$engine" \
            --plugin gtk
    )
}

# The GTK plugin compiles the schemas and sets GSETTINGS_SCHEMA_DIR in its hook.
# Fail if one of the two is missing. Rust code must not set the variable.
check_schema_setup() {
    local hooks="$APPDIR/apprun-hooks"
    if [ ! -f "$APPDIR/usr/share/glib-2.0/schemas/gschemas.compiled" ]; then
        echo "error: compiled GSettings schemas are missing in AppDir" >&2
        exit 1
    fi
    if ! grep -q 'ringomusic-env.sh' "$APPDIR/AppRun"; then
        echo "error: AppRun does not source the ringomusic-env.sh hook" >&2
        exit 1
    fi
    if ! grep -rqs 'GSETTINGS_SCHEMA_DIR' "$hooks"; then
        echo "error: no apprun hook sets GSETTINGS_SCHEMA_DIR" >&2
        exit 1
    fi
}

check_appdir_content() {
    local found
    found="$(find "$APPDIR" -iname '*widevine*' -o -name chrome-sandbox)"
    if [ -n "$found" ]; then
        echo "error: forbidden files in the AppImage:" >&2
        echo "$found" >&2
        exit 1
    fi
    appstreamcli validate --no-net "$APPDIR/usr/share/metainfo/$APP_ID.metainfo.xml"
}

# Builds the AppImage with the static type2 runtime and the update information.
run_appimagetool() {
    local version="$1" output="RingoMusic-$version-x86_64.AppImage"
    mkdir -p "$REPO_ROOT/dist"
    (
        cd "$REPO_ROOT/dist"
        rm -f "$output" "$output.zsync"
        export APPIMAGE_EXTRACT_AND_RUN=1
        ARCH=x86_64 "$TOOLS_DIR/appimagetool-x86_64.AppImage" \
            --runtime-file "$TOOLS_DIR/runtime-x86_64" \
            --updateinformation "$UPDATE_INFO" \
            "$APPDIR" "$output"
    )
    if [ ! -f "$REPO_ROOT/dist/$output.zsync" ]; then
        echo "error: appimagetool did not create $output.zsync" >&2
        exit 1
    fi
    echo "appimage: dist/$output"
}

main() {
    local version
    version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$REPO_ROOT/Cargo.toml" | head -n 1)"
    fetch_all_tools
    prepare_appdir
    run_linuxdeploy
    check_schema_setup
    check_appdir_content
    run_appimagetool "$version"
}

main
