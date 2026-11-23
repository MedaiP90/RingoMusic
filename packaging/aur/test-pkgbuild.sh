#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Builds the AUR package from the local tarball in dist/ and runs namcap on the PKGBUILD and the
# package. Run packaging/build-packages.sh first. Install the result with "pacman -U".
# Usage: packaging/aur/test-pkgbuild.sh [VERSION]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

VERSION="${1:-$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)}"
VERSION="${VERSION#v}"
TARBALL="$REPO_ROOT/dist/ringomusic-${VERSION}-x86_64.tar.gz"
if [ ! -f "$TARBALL" ]; then
    echo "error: $TARBALL is missing. Run packaging/build-packages.sh first." >&2
    exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp packaging/aur/PKGBUILD "$work/PKGBUILD"
cp "$TARBALL" "$work/ringomusic-bin-local.tar.gz"
# Use the local tarball and its checksum. A "-" in the version becomes "_" (pkgver rule).
sed -i \
    -e "s/^pkgver=.*/pkgver=${VERSION//-/_}/" \
    -e "s|^source=.*|source=('ringomusic-bin-local.tar.gz')|" \
    -e "s/^sha256sums=.*/sha256sums=('$(sha256sum "$TARBALL" | cut -d' ' -f1)')/" \
    "$work/PKGBUILD"

cd "$work"
namcap PKGBUILD
makepkg --force --nodeps --noconfirm
namcap ./*.pkg.tar.zst
mkdir -p "$REPO_ROOT/dist"
cp ./*.pkg.tar.zst "$REPO_ROOT/dist/"
echo "package: $(ls "$REPO_ROOT"/dist/ringomusic-bin-*.pkg.tar.zst)"
