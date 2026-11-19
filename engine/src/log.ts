// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import pino from 'pino';

// Logs go to stderr. The protocol transport replaces this later (decision D9).
export const log = pino({ level: process.env['LOG_LEVEL'] ?? 'info' }, pino.destination(2));
