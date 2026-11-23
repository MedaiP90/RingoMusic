#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Post-install script for deb and rpm. A missing tool must not fail the installation.

set -e

if command -v glib-compile-schemas >/dev/null 2>&1; then
    glib-compile-schemas /usr/share/glib-2.0/schemas || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q || true
fi

# The profile exists only in the deb package.
apparmor_active() {
    if command -v aa-enabled >/dev/null 2>&1; then
        aa-enabled --quiet >/dev/null 2>&1
    else
        [ -d /sys/module/apparmor ]
    fi
}

if [ -f /etc/apparmor.d/ringomusic ] && command -v apparmor_parser >/dev/null 2>&1 \
    && apparmor_active; then
    apparmor_parser -r -T -W /etc/apparmor.d/ringomusic || true
fi

exit 0
