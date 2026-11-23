#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Sets the release version in Cargo.toml, Cargo.lock, engine/package.json, and
# engine/package-lock.json. Adds a <release> entry to the metainfo file.
# Usage: scripts/bump-version.sh <version>   (for example 0.9.0-rc.1)
# After the script: edit the <description> of the new <release> entry. It becomes the release notes.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

version="${1:-}"
version="${version#v}"
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]; then
    echo "usage: $0 <version>   (like 1.2.3 or 1.2.3-rc.1)" >&2
    exit 1
fi
metainfo=data/io.github.MedaiP90.RingoMusic.metainfo.xml

# The first "version =" line is the workspace version.
sed -i "0,/^version = \".*\"\$/s//version = \"$version\"/" Cargo.toml
# This also updates package-lock.json. It makes no git tag and no commit.
(cd engine && npm version "$version" --no-git-tag-version --allow-same-version >/dev/null)
# Updates the workspace members in Cargo.lock only. No dependency changes.
cargo update --workspace --offline

if grep -q "<release version=\"$version\"" "$metainfo"; then
    echo "metainfo already has release $version"
else
    sed -i "/<releases>/a\\    <release version=\"$version\" date=\"$(date +%F)\">\\
      <description>\\
        <p>Release $version.</p>\\
      </description>\\
    </release>" "$metainfo"
fi
echo "Version is now $version. Edit the <description> in $metainfo."
