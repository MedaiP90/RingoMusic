// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';

import { ErrorCode } from './error-code.ts';
import { createHandlers, dispatchRequest, QueueErrorWatch } from './hook-handlers.ts';
import { PAGE_METHODS } from './page-methods.ts';

function fakeMusic(overrides: Partial<MusicKitInstance> = {}): MusicKitInstance {
  return { shuffleMode: 1, ...overrides } as MusicKitInstance;
}

void test('handlers / every request example / should have a handler', () => {
  const examples = path.join(import.meta.dirname, '../../protocol/examples');
  const methods = readdirSync(examples)
    .filter((name) => name.startsWith('request.'))
    .map((name) => name.slice('request.'.length, -'.json'.length))
    .filter((method) => method !== 'engine.shutdown' && method !== 'auth.signIn');
  const handlers = Object.keys(createHandlers(fakeMusic()));
  assert.deepEqual([...methods].sort(), [...handlers].sort());
  assert.deepEqual([...PAGE_METHODS].sort(), [...handlers].sort());
});

void test('dispatchRequest / unknown method / should return method_not_found', async () => {
  const response = await dispatchRequest(createHandlers(fakeMusic()), { id: 1, method: 'nope' });
  assert.deepEqual(response, {
    jsonrpc: '2.0',
    id: 1,
    error: { code: ErrorCode.MethodNotFound, message: 'method not found: nope' },
  });
});

void test('dispatchRequest / prototype name / should return method_not_found', async () => {
  const response = await dispatchRequest(createHandlers(fakeMusic()), { id: 1, method: 'toString' });
  assert.ok('error' in response);
});

void test('dispatchRequest / rejected MusicKit call / should map the error', async () => {
  const music = fakeMusic({ play: () => Promise.reject(Object.assign(new Error('no plan'), { status: 403 })) });
  const response = await dispatchRequest(createHandlers(music), { id: 2, method: 'player.play' });
  assert.deepEqual(response, {
    jsonrpc: '2.0',
    id: 2,
    error: { code: ErrorCode.NoSubscription, message: 'no plan' },
  });
});

void test('player.setQueue / valid params / should set the queue and turn shuffle off', async () => {
  const calls: unknown[] = [];
  const music = fakeMusic({
    setQueue: (options) => {
      calls.push(options);
      return Promise.resolve();
    },
  });
  const params = { items: ['i.a', 'i.b'], kind: 'songs', start_index: 1 };
  const response = await dispatchRequest(createHandlers(music), {
    id: 3,
    method: 'player.setQueue',
    params,
  });
  assert.deepEqual(calls, [{ songs: ['i.a', 'i.b'], startWith: 1 }]);
  assert.equal(music.shuffleMode, 0);
  assert.deepEqual(response, { jsonrpc: '2.0', id: 3, result: null });
});

void test('player.seek / position in ms / should call MusicKit with seconds', async () => {
  const seen: number[] = [];
  const music = fakeMusic({
    seekToTime: (seconds) => {
      seen.push(seconds);
      return Promise.resolve();
    },
  });
  await dispatchRequest(createHandlers(music), {
    id: 4,
    method: 'player.seek',
    params: { position_ms: 1500 },
  });
  assert.deepEqual(seen, [1.5]);
});

void test('api.get / response / should return only the data', async () => {
  const music = fakeMusic({ api: { music: () => Promise.resolve({ data: { x: 1 } }) } });
  const response = await dispatchRequest(createHandlers(music), {
    id: 5,
    method: 'api.get',
    params: { path: '/v1/me/library/songs', query: {} },
  });
  assert.deepEqual(response, { jsonrpc: '2.0', id: 5, result: { x: 1 } });
});

const RESTRICTED = '[mk-007] CONTENT_RESTRICTED; Content restricted';

function restrictedMusic(watch: QueueErrorWatch, failures: number): { music: MusicKitInstance; calls: () => number } {
  let calls = 0;
  const music = fakeMusic({
    setQueue: () => {
      calls += 1;
      if (calls <= failures) {
        watch.absorb(RESTRICTED);
      }
      return Promise.resolve();
    },
  });
  return { music, calls: () => calls };
}

const QUEUE_REQUEST = { id: 5, method: 'player.setQueue', params: { items: ['1'], kind: 'songs', start_index: 0 } };

void test('player.setQueue / restricted once / should retry and succeed', async () => {
  const watch = new QueueErrorWatch();
  const { music, calls } = restrictedMusic(watch, 1);
  const response = await dispatchRequest(createHandlers(music, watch, 0), QUEUE_REQUEST);
  assert.equal(calls(), 2);
  assert.deepEqual(response, { jsonrpc: '2.0', id: 5, result: null });
});

void test('player.setQueue / always restricted / should fail after three attempts', async () => {
  const watch = new QueueErrorWatch();
  const { music, calls } = restrictedMusic(watch, 99);
  const response = await dispatchRequest(createHandlers(music, watch, 0), QUEUE_REQUEST);
  assert.equal(calls(), 3);
  assert.ok('error' in response);
});

void test('QueueErrorWatch / error outside setQueue / should not absorb it', () => {
  assert.equal(new QueueErrorWatch().absorb(RESTRICTED), false);
});
