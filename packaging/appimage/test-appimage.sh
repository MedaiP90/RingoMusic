#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Runs "RingoMusic-<version>-x86_64.AppImage --version" in minimal containers.
# Containers have no FUSE, so the AppImage extracts itself. Needs Docker.
# Usage: packaging/appimage/test-appimage.sh [IMAGE...]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGES=("$@")
if [ ${#IMAGES[@]} -eq 0 ]; then
    IMAGES=(debian:13 fedora:latest opensuse/tumbleweed archlinux:latest)
fi

appimage="$(ls "$REPO_ROOT"/dist/RingoMusic-*-x86_64.AppImage | head -n 1)"
failed=0
for image in "${IMAGES[@]}"; do
    echo "== $image"
    # The image gets no extra package. The AppImage must bring its libraries.
    if ! docker run --rm -e APPIMAGE_EXTRACT_AND_RUN=1 \
        -v "$appimage:/ringomusic.AppImage:ro" "$image" /ringomusic.AppImage --version; then
        echo "FAILED: $image" >&2
        failed=1
    fi
done
exit "$failed"
