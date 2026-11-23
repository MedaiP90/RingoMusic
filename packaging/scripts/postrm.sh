#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Post-remove script for deb and rpm. A missing tool must not fail the removal.

# On a deb removal, the profile file is still on disk (dpkg keeps config files).
if [ "$1" = "remove" ] && [ -f /etc/apparmor.d/ringomusic ] \
    && command -v apparmor_parser >/dev/null 2>&1 && [ -d /sys/module/apparmor ]; then
    apparmor_parser -R /etc/apparmor.d/ringomusic >/dev/null 2>&1 || true
fi

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

exit 0
