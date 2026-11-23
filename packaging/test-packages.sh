#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Installs the packages from dist/ in containers, runs "ringomusic --version", removes the
# package, and checks that no file stays in /usr. Needs Docker.
# Usage: packaging/test-packages.sh [IMAGE...]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGES=("$@")
if [ ${#IMAGES[@]} -eq 0 ]; then
    IMAGES=(debian:13 ubuntu:26.04 fedora:43 fedora:44)
fi

# Container script. $PKG_KIND is "deb" or "rpm".
read -r -d '' CONTAINER_SCRIPT <<'SCRIPT' || true
set -eu
if [ "$PKG_KIND" = deb ]; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq adwaita-icon-theme >/dev/null   # a normal desktop has the icon theme
    apt-get install -y -qq /dist/*.deb >/dev/null
    dpkg -L ringomusic | sort > /files.txt
else
    dnf install -y -q adwaita-icon-theme >/dev/null
    dnf install -y -q /dist/*.rpm >/dev/null
    rpm -ql ringomusic | sort > /files.txt
fi
ringomusic --version
stat -c '%a %U' /usr/lib/ringomusic/engine/chrome-sandbox
if find / -xdev -iname '*widevine*' 2>/dev/null | grep -q .; then echo "Widevine file found"; exit 1; fi
missing="$(ldd /usr/lib/ringomusic/engine/ringomusic-engine /usr/bin/ringomusic | grep 'not found' || true)"
if [ -n "$missing" ]; then echo "missing libraries:"; echo "$missing"; exit 1; fi
if [ "$PKG_KIND" = deb ]; then apt-get purge -y -qq ringomusic >/dev/null; else dnf remove -y -q ringomusic >/dev/null; fi
# Check that no package file stays, and that no generated cache still names the app.
leftover="$(while read -r path; do
    if [ -f "$path" ] || [ -L "$path" ]; then echo "$path"; fi
done < /files.txt)"
[ -d /usr/lib/ringomusic ] && leftover="$leftover /usr/lib/ringomusic"
if grep -rl RingoMusic /usr/share/glib-2.0/schemas/gschemas.compiled /usr/share/icons/hicolor/icon-theme.cache \
    /usr/share/applications/mimeinfo.cache 2>/dev/null | grep -q .; then
    leftover="$leftover (stale cache)"
fi
if [ -n "$(echo "$leftover" | tr -d ' ')" ]; then echo "files left after removal: $leftover"; exit 1; fi
echo "OK"
SCRIPT

for image in "${IMAGES[@]}"; do
    kind=deb
    case "$image" in fedora:* | opensuse/*) kind=rpm ;; esac
    echo "== $image ($kind)"
    docker run --rm -v "$REPO_ROOT/dist:/dist:ro" -e PKG_KIND="$kind" "$image" \
        sh -c "$CONTAINER_SCRIPT"
done
