// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { LineFramer } from './framer.ts';

void test('framer / line split in two chunks / should return it after the second chunk', () => {
  const framer = new LineFramer();
  assert.deepEqual(framer.push('{"a":'), []);
  assert.deepEqual(framer.push('1}\n'), ['{"a":1}']);
});

void test('framer / two lines in one chunk / should return both and ignore empty lines', () => {
  const framer = new LineFramer();
  assert.deepEqual(framer.push('one\n\ntwo\n'), ['one', 'two']);
});

void test('framer / trailing partial line / should keep it for the next chunk', () => {
  const framer = new LineFramer();
  assert.deepEqual(framer.push('one\ntw'), ['one']);
  assert.deepEqual(framer.push('o\n'), ['two']);
});
