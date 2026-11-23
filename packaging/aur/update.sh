#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Updates PKGBUILD and .SRCINFO for a published GitHub release: pkgver, pkgrel=1, sha256sums.
# Usage: packaging/aur/update.sh <version>   (like 1.0.0 or 0.9.0-rc.1)
# Then copy PKGBUILD and .SRCINFO to the AUR clone, commit, and push.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

version="${1:-}"
version="${version#v}"
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]; then
    echo "usage: $0 <version>   (like 1.2.3 or 1.2.3-rc.1)" >&2
    exit 1
fi

url="https://github.com/MedaiP90/RingoMusic/releases/download/v${version}"
tarball="ringomusic-${version}-x86_64.tar.gz"
checksum="$(curl -fsSL "$url/SHA256SUMS" | awk -v name="$tarball" '$2 == name { print $1 }')"
if ! [[ "$checksum" =~ ^[0-9a-f]{64}$ ]]; then
    echo "error: no checksum for $tarball in $url/SHA256SUMS" >&2
    exit 1
fi

# pacman forbids "-" in pkgver, so "0.9.0-rc.1" becomes "0.9.0_rc.1".
sed -i \
    -e "s/^pkgver=.*/pkgver=${version//-/_}/" \
    -e "s/^pkgrel=.*/pkgrel=1/" \
    -e "s/^sha256sums=.*/sha256sums=('$checksum')/" \
    PKGBUILD
makepkg --printsrcinfo > .SRCINFO
echo "PKGBUILD and .SRCINFO are now at ${version}."
