#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Builds RingoMusic and creates the common install tree in stage/.
# Usage: packaging/stage.sh [PREFIX]   (default PREFIX: /usr)
set -euo pipefail

# shellcheck source=packaging/castlabs-lib.sh
source "$(dirname "$0")/castlabs-lib.sh"

APP_ID="io.github.MedaiP90.RingoMusic"
PREFIX="${1:-/usr}"
STAGE="$REPO_ROOT/stage"
DEST="$STAGE$PREFIX"
ENGINE_DIR="$DEST/lib/ringomusic/engine"

cd "$REPO_ROOT"
rm -rf "$STAGE"

build_binary() {
    cargo build --release --locked -p ringomusic
    install -Dm755 target/release/ringomusic "$DEST/bin/ringomusic"
    strip --strip-unneeded "$DEST/bin/ringomusic"
}

build_engine_bundles() {
    (cd engine && npm ci && npm run build)
}

# Fails if the bundles require a module that is neither electron nor a Node built-in.
check_engine_externals() {
    local bad
    # shellcheck disable=SC2016 # The JavaScript code has its own template literals.
    bad="$(node -e '
        const { builtinModules } = require("node:module");
        const fs = require("node:fs");
        const found = new Set();
        for (const f of fs.readdirSync("engine/dist")) {
            if (!f.endsWith(".js")) continue;
            const text = fs.readFileSync(`engine/dist/${f}`, "utf8");
            for (const m of text.matchAll(/\brequire\(\s*"([^"]+)"\s*\)/g)) found.add(m[1]);
        }
        const bad = [...found].filter((m) => m !== "electron" && !m.startsWith(".")
            && !m.startsWith("/") && !builtinModules.includes(m.replace(/^node:/, "")));
        console.log(bad.join(" "));
    ')"
    if [ -n "$bad" ]; then
        echo "error: engine bundles require packages that are not bundled: $bad" >&2
        exit 1
    fi
}

install_electron() {
    local zip expected actual
    zip="$(castlabs_download)"
    expected="$(cut -d' ' -f1 "$CASTLABS_SHA_FILE")"
    actual="$(sha256sum "$zip" | cut -d' ' -f1)"
    if [ "$expected" != "$actual" ]; then
        echo "error: SHA-256 of $zip does not match packaging/castlabs.sha256" >&2
        echo "  expected $expected" >&2
        echo "  actual   $actual" >&2
        exit 1
    fi
    mkdir -p "$ENGINE_DIR"
    unzip -q "$zip" -d "$ENGINE_DIR"
    mv "$ENGINE_DIR/electron" "$ENGINE_DIR/ringomusic-engine"
    rm -f "$ENGINE_DIR/resources/default_app.asar"
}

install_engine_app() {
    local app="$ENGINE_DIR/resources/app"
    mkdir -p "$app"
    cp -r engine/dist "$app/dist"
    # The app manifest keeps only the fields that Electron needs.
    node -e '
        const p = require("./engine/package.json");
        const { name, version, main, license } = p;
        console.log(JSON.stringify({ name, version, main, license }, null, 2));
    ' > "$app/package.json"
}

install_data_files() {
    install -Dm644 "data/$APP_ID.desktop" "$DEST/share/applications/$APP_ID.desktop"
    install -Dm644 "data/$APP_ID.metainfo.xml" "$DEST/share/metainfo/$APP_ID.metainfo.xml"
    install -Dm644 "data/icons/$APP_ID.svg" \
        "$DEST/share/icons/hicolor/scalable/apps/$APP_ID.svg"
    install -Dm644 "data/icons/$APP_ID-symbolic.svg" \
        "$DEST/share/icons/hicolor/symbolic/apps/$APP_ID-symbolic.svg"
    install -Dm644 "data/$APP_ID.gschema.xml" "$DEST/share/glib-2.0/schemas/$APP_ID.gschema.xml"
    install -Dm644 LICENSE "$DEST/share/licenses/ringomusic/LICENSE"
}

compile_translations() {
    local po lang
    shopt -s nullglob
    for po in po/*.po; do
        lang="$(basename "$po" .po)"
        mkdir -p "$DEST/share/locale/$lang/LC_MESSAGES"
        msgfmt --check --output-file="$DEST/share/locale/$lang/LC_MESSAGES/ringomusic.mo" "$po"
    done
    shopt -u nullglob
}

# Packages must not have group-write or other-write bits (the zip uses the umask of the builder).
# chrome-sandbox gets the SUID bit here. The package tool sets the owner root.
normalize_permissions() {
    chmod -R go-w "$STAGE"
    chmod 4755 "$ENGINE_DIR/chrome-sandbox"
}

check_no_widevine() {
    local found
    found="$(find "$STAGE" -iname '*widevine*')"
    if [ -n "$found" ]; then
        echo "error: Widevine files in the install tree:" >&2
        echo "$found" >&2
        exit 1
    fi
}

build_binary
build_engine_bundles
check_engine_externals
install_electron
install_engine_app
install_data_files
compile_translations
normalize_permissions
check_no_widevine
echo "install tree: $STAGE ($(du -sh "$STAGE" | cut -f1))"
