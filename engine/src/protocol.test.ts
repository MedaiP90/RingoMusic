// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { test } from 'node:test';

import type { Notification, Request } from './protocol.ts';

const examplesDirectory = new URL('../../protocol/examples/', import.meta.url);

// The `Record` types make the compiler fail when the protocol gets a new method
// that has no entry here.
const requestMethods: Record<Request['method'], true> = {
  'auth.status': true,
  'auth.signIn': true,
  'auth.signOut': true,
  'player.setQueue': true,
  'player.play': true,
  'player.pause': true,
  'player.stop': true,
  'player.next': true,
  'player.previous': true,
  'player.seek': true,
  'player.setVolume': true,
  'player.setRepeat': true,
  'api.get': true,
  'engine.shutdown': true,
};

const notificationMethods: Record<Notification['method'], true> = {
  'engine.hello': true,
  'engine.ready': true,
  'engine.incompatible': true,
  'engine.log': true,
  'auth.changed': true,
  'player.state': true,
  'player.nowPlaying': true,
  'player.position': true,
  error: true,
};

function hasKnownMethod(value: unknown, known: Record<string, true>): boolean {
  return (
    typeof value === 'object' &&
    value !== null &&
    'method' in value &&
    typeof value.method === 'string' &&
    Object.hasOwn(known, value.method)
  );
}

function isRequest(value: unknown): value is Request {
  return hasKnownMethod(value, requestMethods) && 'id' in (value as object);
}

function isNotification(value: unknown): value is Notification {
  return hasKnownMethod(value, notificationMethods) && !('id' in (value as object));
}

function readExamples(prefix: string): Map<string, unknown> {
  const examples = new Map<string, unknown>();
  for (const file of readdirSync(examplesDirectory)) {
    if (file.startsWith(`${prefix}.`)) {
      examples.set(file, JSON.parse(readFileSync(new URL(file, examplesDirectory), 'utf8')));
    }
  }
  return examples;
}

void test('protocol / request examples / should match the Request type guard', () => {
  const examples = readExamples('request');
  assert.equal(examples.size, Object.keys(requestMethods).length);
  for (const [file, value] of examples) {
    assert.ok(isRequest(value), file);
    assert.equal(file, `request.${value.method}.json`);
  }
});

void test('protocol / notification examples / should match the Notification type guard', () => {
  const examples = readExamples('notification');
  assert.equal(examples.size, Object.keys(notificationMethods).length);
  for (const [file, value] of examples) {
    assert.ok(isNotification(value), file);
    assert.equal(file, `notification.${value.method}.json`);
  }
});
