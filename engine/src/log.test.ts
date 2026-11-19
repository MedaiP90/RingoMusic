// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { log } from './log.ts';

void test('log / when created / should expose the info method', () => {
  assert.equal(typeof log.info, 'function');
});
