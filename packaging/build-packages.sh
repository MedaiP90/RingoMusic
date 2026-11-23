#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Builds the .deb, the .rpm, and the .tar.gz from the install tree (run packaging/stage.sh first).
# Usage: packaging/build-packages.sh [VERSION]
# VERSION defaults to the workspace version. A leading "v" is removed.
# Environment: MAINTAINER, NFPM (nfpm command, default: nfpm).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

VERSION="${1:-$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)}"
export VERSION="${VERSION#v}"
export MAINTAINER="${MAINTAINER:-RingoMusic contributors <noreply@users.noreply.github.com>}"
NFPM="${NFPM:-nfpm}"

if [ ! -d stage/usr ]; then
    echo "error: stage/usr is missing. Run packaging/stage.sh first." >&2
    exit 1
fi

rm -rf dist
mkdir -p dist
# The targets are file names. nfpm would add the revision "-1" to the deb name, and "~" to the
# rpm name of a pre-release. The metadata inside the packages still has the correct versions
# (0.9.0~rc.1 for 0.9.0-rc.1).
$NFPM package --config packaging/nfpm.yaml --packager deb \
    --target "dist/ringomusic_${VERSION}_amd64.deb"
$NFPM package --config packaging/nfpm.yaml --packager rpm \
    --target "dist/ringomusic-${VERSION}-1.x86_64.rpm"

# The tarball keeps the usr/ paths. The AUR package uses it. The options make the archive stable:
# owner root, files sorted by name, no file name or time stamp in the gzip header.
tar_args=(--sort=name --owner=0 --group=0 --numeric-owner)
if [ -n "${SOURCE_DATE_EPOCH:-}" ]; then
    tar_args+=(--mtime="@${SOURCE_DATE_EPOCH}")
fi
tar "${tar_args[@]}" -cf - -C stage usr | gzip -n -9 > "dist/ringomusic-${VERSION}-x86_64.tar.gz"

# Check the file names from plan section 1.1.
for name in "ringomusic_${VERSION}_amd64.deb" "ringomusic-${VERSION}-1.x86_64.rpm" \
    "ringomusic-${VERSION}-x86_64.tar.gz"; do
    if [ ! -f "dist/$name" ]; then
        echo "error: expected dist/$name, found:" >&2
        ls dist >&2
        exit 1
    fi
done
ls -lh dist
