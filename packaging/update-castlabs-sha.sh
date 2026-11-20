#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Downloads the pinned CastLabs Electron archive and writes its SHA-256 to
# packaging/castlabs.sha256. Run it after you change engine/CASTLABS_VERSION.md.
set -euo pipefail

# shellcheck source=packaging/castlabs-lib.sh
source "$(dirname "$0")/castlabs-lib.sh"

# A cached file for an old pin has another name, so it is not reused.
zip="$(castlabs_download)"
hash="$(sha256sum "$zip" | cut -d' ' -f1)"
echo "$hash  $(castlabs_zip_name)" > "$CASTLABS_SHA_FILE"
echo "wrote $CASTLABS_SHA_FILE"
