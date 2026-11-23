#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Runs "RingoMusic-<version>-x86_64.AppImage --version" in containers.
# Containers have no FUSE, so the AppImage extracts itself. Needs Docker.
# linuxdeploy does not bundle the base libraries that every desktop has (fonts, X11, Wayland,
# GL). The script installs them first. The AppImage must bring GTK 4, libadwaita, and the rest.
# Usage: packaging/appimage/test-appimage.sh [IMAGE...]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGES=("$@")
if [ ${#IMAGES[@]} -eq 0 ]; then
    IMAGES=(debian:13 fedora:latest archlinux:latest)
fi

# Container script. Installs the desktop base libraries, then runs the AppImage.
read -r -d '' CONTAINER_SCRIPT <<'SCRIPT' || true
set -eu
if command -v apt-get >/dev/null; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends \
        libfontconfig1 libfreetype6 libharfbuzz0b libfribidi0 libx11-6 libxcb1 libwayland-client0 \
        libegl1 libgl1 libdrm2 libexpat1 libcomerr2 libgbm1 libasound2t64 >/dev/null
elif command -v dnf >/dev/null; then
    dnf install -y -q fontconfig freetype harfbuzz fribidi libX11 libxcb libwayland-client \
        libglvnd-egl libglvnd-glx libdrm expat libcom_err mesa-libgbm alsa-lib >/dev/null
elif command -v pacman >/dev/null; then
    pacman -Sy --noconfirm --needed fontconfig freetype2 harfbuzz fribidi libx11 libxcb wayland \
        libglvnd libdrm expat e2fsprogs mesa alsa-lib >/dev/null
fi
/ringomusic.AppImage --version
SCRIPT

appimage="$(ls "$REPO_ROOT"/dist/RingoMusic-*-x86_64.AppImage | head -n 1)"
# The CI artifact download does not keep the executable bit.
chmod +x "$appimage"
failed=0
for image in "${IMAGES[@]}"; do
    echo "== $image"
    if ! docker run --rm -e APPIMAGE_EXTRACT_AND_RUN=1 \
        -v "$appimage:/ringomusic.AppImage:ro" "$image" sh -c "$CONTAINER_SCRIPT"; then
        echo "FAILED: $image" >&2
        failed=1
    fi
done
exit "$failed"
