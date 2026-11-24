// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import pino from 'pino';

import type { Notification } from './protocol.ts';

type LogNotification = Extract<Notification, { method: 'engine.log' }>;
type LogParams = LogNotification['params'];
type Forwarder = (notification: LogNotification) => void;

const LEVEL_NAMES: Record<number, LogParams['level']> = {
  10: 'trace',
  20: 'debug',
  30: 'info',
  40: 'warn',
  50: 'error',
  60: 'error',
};

let forwarder: Forwarder | undefined;
let flusher: (() => Promise<void>) | undefined;

// Sends logs to the socket after it connects (decision D9). Before that, logs go to stderr.
// `flush` resolves when the socket has written all queued data.
export function forwardLogs(send: Forwarder, flush: () => Promise<void>): void {
  forwarder = send;
  flusher = flush;
}

// Waits until the socket has sent all queued logs, or until the timeout ends.
export async function flushLogs(timeoutMs: number): Promise<void> {
  if (!flusher) {
    return;
  }
  let timer: NodeJS.Timeout | undefined;
  const timeout = new Promise<void>((resolve) => {
    timer = setTimeout(resolve, timeoutMs);
  });
  await Promise.race([flusher(), timeout]);
  clearTimeout(timer);
}

function toNotification(line: string): LogNotification {
  const { level, msg, ...fields } = JSON.parse(line) as Record<string, unknown>;
  return {
    jsonrpc: '2.0',
    method: 'engine.log',
    params: {
      level: LEVEL_NAMES[level as number] ?? 'info',
      message: typeof msg === 'string' ? msg : '',
      fields,
    },
  };
}

const sink = {
  write(line: string): void {
    if (forwarder) {
      forwarder(toNotification(line));
    } else {
      process.stderr.write(line);
    }
  },
};

export const log = pino({ level: process.env['LOG_LEVEL'] ?? 'info', base: null, timestamp: false }, sink);
