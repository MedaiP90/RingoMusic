#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Downloads the pinned CastLabs Electron archive and writes its SHA-256 to
# packaging/castlabs.sha256, and updates the URL and the SHA-256 in the Flatpak
# manifest. Run it after you change engine/CASTLABS_VERSION.md.
set -euo pipefail

# shellcheck source=packaging/castlabs-lib.sh
source "$(dirname "$0")/castlabs-lib.sh"

# A cached file for an old pin has another name, so it is not reused.
tag="$(castlabs_tag)"
zip="$(castlabs_download)"
hash="$(sha256sum "$zip" | cut -d' ' -f1)"
echo "$hash  $(castlabs_zip_name)" > "$CASTLABS_SHA_FILE"
echo "wrote $CASTLABS_SHA_FILE"

# The manifest pins the same archive as a source.
manifest="$PACKAGING_DIR/flatpak/io.github.MedaiP90.RingoMusic.yml"
zip_name="$(castlabs_zip_name)"
url="https://github.com/castlabs/electron-releases/releases/download/${tag//+/%2B}/${zip_name//+/%2B}"
sed -i \
    -e "s|^\( *url: \).*electron-releases.*|\1$url|" \
    -e "s|^\( *sha256: \).*|\1$hash|" \
    "$manifest"
echo "wrote $manifest"
