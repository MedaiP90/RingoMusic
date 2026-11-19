// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Test socket server: prints each line that the engine sends.
// Usage: node scripts/test-socket.mjs <socket-path>

import fs from 'node:fs';
import net from 'node:net';
import process from 'node:process';

const socketPath = process.argv[2];
if (!socketPath) {
  process.stderr.write('usage: node scripts/test-socket.mjs <socket-path>\n');
  process.exit(2);
}
fs.rmSync(socketPath, { force: true });

net
  .createServer((socket) => {
    socket.setEncoding('utf8');
    socket.on('data', (chunk) => process.stdout.write(chunk));
  })
  .listen(socketPath, () => process.stderr.write(`listening on ${socketPath}\n`));
