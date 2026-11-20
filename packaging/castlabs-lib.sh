#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Shared functions for the CastLabs Electron archive. Source this file.

PACKAGING_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$PACKAGING_DIR")"
CASTLABS_SHA_FILE="$PACKAGING_DIR/castlabs.sha256"

# Prints the pinned tag, for example "v45.2.1+wvcus".
castlabs_tag() {
    sed -n 's/^- Tag: `\(.*\)`$/\1/p' "$REPO_ROOT/engine/CASTLABS_VERSION.md"
}

# Prints the archive file name for the pinned tag.
castlabs_zip_name() {
    echo "electron-$(castlabs_tag)-linux-x64.zip"
}

# Downloads the archive into the cache and prints its path.
# A cached file is reused. The download goes to a temporary name first.
castlabs_download() {
    local tag name cache url
    tag="$(castlabs_tag)"
    name="$(castlabs_zip_name)"
    cache="${RINGO_BUILD_CACHE:-$HOME/.cache/ringomusic-build}"
    mkdir -p "$cache"
    if [ ! -f "$cache/$name" ]; then
        url="https://github.com/castlabs/electron-releases/releases/download/${tag//+/%2B}/${name//+/%2B}"
        echo "downloading $url" >&2
        curl --fail --location --retry 3 --silent --show-error --output "$cache/$name.part" "$url" >&2
        mv "$cache/$name.part" "$cache/$name"
    fi
    echo "$cache/$name"
}
