// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { flushLogs, forwardLogs, log } from './log.ts';

void test('log / when an error is logged under err / should forward its message and stack', () => {
  const sent: unknown[] = [];
  forwardLogs(
    (notification) => sent.push(notification),
    () => Promise.resolve(),
  );
  log.error({ err: new Error('boom') }, 'failed');
  const { params } = sent[0] as { params: { fields: { err: { message: string; stack: string } } } };
  assert.equal(params.fields.err.message, 'boom');
  assert.match(params.fields.err.stack, /Error: boom/);
});

void test('flushLogs / when the flush never ends / should resolve after the timeout', async () => {
  forwardLogs(
    () => {},
    () => new Promise<void>(() => {}),
  );
  await flushLogs(10);
});

void test('log / when created / should expose the info method', () => {
  assert.equal(typeof log.info, 'function');
});
