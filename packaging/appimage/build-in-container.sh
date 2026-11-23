#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Builds the AppImage inside a debian:13 container. Use it on any host that is
# not Debian 13: the AppImage bundles the libraries of the build system, and
# its glibc must be old. Libraries of newer systems can use RELR relocations,
# which the patchelf of linuxdeploy corrupts. Needs Docker.
# Usage: packaging/appimage/build-in-container.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CACHE="${RINGO_BUILD_CACHE:-$HOME/.cache/ringomusic-build}"
NODE_VERSION="$(cat "$REPO_ROOT/engine/.nvmrc")"
mkdir -p "$CACHE" "$REPO_ROOT/dist"

# Container script. The source is copied, so the host target/ and stage/ stay untouched.
read -r -d '' CONTAINER_SCRIPT <<'SCRIPT' || true
set -eu
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends \
    build-essential ca-certificates curl git pkg-config unzip xz-utils file \
    libgtk-4-dev libadwaita-1-dev libdbus-1-dev libglib2.0-bin gettext appstream \
    librsvg2-common >/dev/null
# Runtime libraries of the Electron engine. linuxdeploy bundles them from this system.
apt-get install -y -qq --no-install-recommends \
    libnss3 libnspr4 libatk1.0-0t64 libatk-bridge2.0-0t64 libcups2t64 libdrm2 libgbm1 \
    libasound2t64 libxkbcommon0 libxcomposite1 libxdamage1 libxfixes3 libxrandr2 \
    libpango-1.0-0 libcairo2 libgtk-3-0t64 libx11-6 libxext6 libxcb1 >/dev/null
# .nvmrc has only the major version. Use the newest release of it.
base="https://nodejs.org/dist/latest-v$NODE_VERSION.x"
node_dir="$(curl -fsSL "$base/SHASUMS256.txt" | sed -n 's/.*\(node-v[0-9.]*-linux-x64\)\.tar\.xz$/\1/p' | head -n 1)"
if [ ! -d "/cache/$node_dir" ]; then
    curl -fsSL "$base/$node_dir.tar.xz" | tar -xJ -C /cache
fi
export PATH="/cache/$node_dir/bin:$HOME/.cargo/bin:$PATH"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null 2>&1
mkdir /build
tar -C /src --exclude=./target --exclude=./stage --exclude=./AppDir --exclude=./dist \
    --exclude=./engine/node_modules -cf - . | tar -C /build -xf -
cd /build
export RINGO_BUILD_CACHE=/cache
packaging/appimage/build-appimage.sh
cp dist/RingoMusic-* /out/
chown "$HOST_UID:$HOST_GID" /out/RingoMusic-*
SCRIPT

docker run --rm \
    -e NODE_VERSION="$NODE_VERSION" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
    -v "$REPO_ROOT:/src:ro" -v "$CACHE:/cache" -v "$REPO_ROOT/dist:/out" \
    debian:13 bash -c "$CONTAINER_SCRIPT"
