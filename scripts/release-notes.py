#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
"""Print the release notes for a version as Markdown.

The notes come from the <description> of the matching <release> in the metainfo file.
If it has none, the script uses the matching section of CHANGELOG.md ("## <version>" or
"## [<version>]").

Usage: scripts/release-notes.py <version>
"""
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

METAINFO = Path("data/io.github.MedaiP90.RingoMusic.metainfo.xml")
CHANGELOG = Path("CHANGELOG.md")


def text_of(element):
    """Return the text of an element with its inline children, on one line."""
    return " ".join("".join(element.itertext()).split())


def notes_from_metainfo(version):
    for release in ET.parse(METAINFO).getroot().iter("release"):
        if release.get("version") != version:
            continue
        description = release.find("description")
        if description is None:
            return ""
        blocks = []
        for child in description:
            if child.tag == "p":
                blocks.append(text_of(child))
            elif child.tag in ("ul", "ol"):
                blocks.append("\n".join(f"- {text_of(item)}" for item in child.iter("li")))
        return "\n\n".join(block for block in blocks if block)
    return ""


def notes_from_changelog(version):
    if not CHANGELOG.exists():
        return ""
    heading = re.compile(rf"^##\s+\[?{re.escape(version)}\]?(\s|$)")
    lines, found = [], False
    for line in CHANGELOG.read_text(encoding="utf-8").splitlines():
        if found and line.startswith("## "):
            break
        if found:
            lines.append(line)
        elif heading.match(line):
            found = True
    return "\n".join(lines).strip()


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    version = sys.argv[1].removeprefix("v")
    notes = notes_from_metainfo(version) or notes_from_changelog(version)
    if not notes:
        sys.exit(f"error: no release notes for {version} in the metainfo file or CHANGELOG.md")
    print(notes)


if __name__ == "__main__":
    main()
