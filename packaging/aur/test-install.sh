#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Runs inside an Arch Linux container as root. Builds the package from the tarball in dist/ with
# the PKGBUILD, installs it with pacman, runs "ringomusic --version", removes it, and checks that
# no package file stays. Needs the repository in the current directory.
# Usage: packaging/aur/test-install.sh VERSION
set -euo pipefail

version="${1:?usage: $0 VERSION}"

pacman -Syu --noconfirm --needed base-devel namcap git >/dev/null
# makepkg refuses to run as root.
useradd --create-home builder
chown -R builder .
runuser -u builder -- packaging/aur/test-pkgbuild.sh "$version"

pacman -U --noconfirm dist/ringomusic-bin-*.pkg.tar.zst
pacman -Ql ringomusic-bin | cut -d' ' -f2 > /files.txt
ringomusic --version
pacman -R --noconfirm ringomusic-bin
leftover="$(while read -r path; do
    if [ -f "$path" ] || [ -L "$path" ]; then echo "$path"; fi
done < /files.txt)"
if [ -n "$leftover" ]; then
    echo "files left after removal: $leftover"
    exit 1
fi
echo "OK"
