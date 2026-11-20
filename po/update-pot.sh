#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Extracts the translatable strings into po/ringomusic.pot.
# xgettext reads Rust with --language=Rust (GNU gettext 0.24 and later).
# Limit: it finds only literal arguments of gettext() and ngettext(), so a
# string inside a macro or a constant is not found.
set -eu

# A fixed locale keeps the sort order and the messages the same on every machine.
export LC_ALL=C

cd "$(dirname "$0")/.."

# Every file with translatable strings must be listed.
missing=$(grep -rlE 'n?gettext\(' crates --include='*.rs' | sort | diff - po/POTFILES.in || true)
if [ -n "$missing" ]; then
    echo "po/POTFILES.in is out of date:" >&2
    echo "$missing" >&2
    exit 1
fi

xgettext \
    --language=Rust \
    --from-code=UTF-8 \
    --keyword=gettext \
    --keyword=ngettext:1,2 \
    --add-location=file \
    --sort-by-file \
    --package-name=ringomusic \
    --copyright-holder='RingoMusic contributors' \
    --msgid-bugs-address='https://github.com/MedaiP90/RingoMusic/issues' \
    --files-from=po/POTFILES.in \
    --output=po/ringomusic.pot

# A fixed date keeps the file identical between runs.
sed -i 's/^"POT-Creation-Date: .*\\n"$/"POT-Creation-Date: 1970-01-01 00:00+0000\\n"/' po/ringomusic.pot
