#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# Extracts the translatable strings into po/ringomusic.pot.
# xgettext picks the language from the file extension: Rust (GNU gettext 0.24
# and later), desktop files, and AppStream metainfo files.
# Limit: it finds only literal arguments of gettext() and ngettext(), so a
# string inside a macro or a constant is not found.
set -euo pipefail

# A fixed locale keeps the sort order and the messages the same on every machine.
export LC_ALL=C

cd "$(dirname "$0")/.."

# Every Rust file with translatable strings must be listed.
missing=$(grep -rlE 'n?gettext\(' crates --include='*.rs' | sort | diff - <(grep '^crates/' po/POTFILES.in) || true)
if [ -n "$missing" ]; then
    echo "po/POTFILES.in is out of date:" >&2
    echo "$missing" >&2
    exit 1
fi

# Options shared by both runs.
common=(
    --from-code=UTF-8 \
    --keyword=gettext \
    --keyword=ngettext:1,2 \
    --add-location=file \
    --sort-by-file \
    --package-name=ringomusic \
    --copyright-holder='RingoMusic contributors' \
    --msgid-bugs-address='https://github.com/MedaiP90/RingoMusic/issues' \
)

# The metainfo file gets its own ITS rules. The rules that gettext ships differ
# between versions: some skip the release notes, some skip the screenshot captions.
xgettext "${common[@]}" \
    --files-from=<(grep -v '\.metainfo\.xml$' po/POTFILES.in) \
    --output=po/ringomusic.pot
xgettext "${common[@]}" --join-existing --its=po/metainfo.its \
    --output=po/ringomusic.pot \
    data/io.github.MedaiP90.RingoMusic.metainfo.xml

# A fixed date keeps the file identical between runs.
sed -i 's/^"POT-Creation-Date: .*\\n"$/"POT-Creation-Date: 1970-01-01 00:00+0000\\n"/' po/ringomusic.pot
