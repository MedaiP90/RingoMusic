# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 RingoMusic contributors
#
# AppRun hook, sourced after the GTK plugin hook. The plugin forces the X11
# backend and a fixed theme. libadwaita must follow the Wayland session and the
# system style, so the hook removes both variables.
unset GDK_BACKEND GTK_THEME
