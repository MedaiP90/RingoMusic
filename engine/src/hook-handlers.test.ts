// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';

import { ErrorCode } from './error-code.ts';
import { createHandlers, dispatchRequest } from './hook-handlers.ts';
import { PAGE_METHODS } from './page-methods.ts';

function fakeMusic(overrides: Partial<MusicKitInstance> = {}): MusicKitInstance {
  return { shuffleMode: 1, ...overrides } as MusicKitInstance;
}

void test('handlers / every request example / should have a handler', () => {
  const examples = path.join(import.meta.dirname, '../../protocol/examples');
  const methods = readdirSync(examples)
    .filter((name) => name.startsWith('request.'))
    .map((name) => name.slice('request.'.length, -'.json'.length))
    .filter((method) => !['engine.shutdown', 'auth.signIn'].includes(method));
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

void test('player.playNext, playLater, remove / valid params / should edit the upcoming items', async () => {
  const calls: unknown[] = [];
  const music = fakeMusic({
    playNext: (options) => {
      calls.push(['next', options]);
      return Promise.resolve();
    },
    playLater: (options) => {
      calls.push(['later', options]);
      return Promise.resolve();
    },
    queue: { remove: (index) => calls.push(['remove', index]) },
  });
  const handlers = createHandlers(music);
  const items = ['i.a', 'i.b'];
  await dispatchRequest(handlers, { id: 1, method: 'player.playNext', params: { items, kind: 'songs' } });
  await dispatchRequest(handlers, { id: 2, method: 'player.playLater', params: { items, kind: 'songs' } });
  const response = await dispatchRequest(handlers, { id: 3, method: 'player.remove', params: { index: 4 } });
  assert.deepEqual(calls, [
    ['next', { songs: items }],
    ['later', { songs: items }],
    ['remove', 4],
  ]);
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

interface FetchCall {
  url: string;
  init: RequestInit;
}

// A fetch stub that records its calls and answers with one fixed response.
function stubFetch(status: number, body: string): { fetchFn: typeof fetch; calls: FetchCall[] } {
  const calls: FetchCall[] = [];
  const fetchFn = ((url: URL, init: RequestInit) => {
    calls.push({ url: url.toString(), init });
    return Promise.resolve(new Response(body === '' ? null : body, { status }));
  }) as typeof fetch;
  return { fetchFn, calls };
}

const TOKENS = { developerToken: 'dev', musicUserToken: 'user' };

function writeRequest(id: number, params: unknown): { id: number; method: string; params: unknown } {
  return { id, method: 'api.write', params };
}

void test('api.write / body and JSON response / should send the request and return the JSON', async () => {
  const { fetchFn, calls } = stubFetch(200, '{"data":[{"id":"1"}]}');
  const handlers = createHandlers(fakeMusic(TOKENS), fetchFn);
  const body = { type: 'rating', attributes: { value: -1 } };
  const response = await dispatchRequest(
    handlers,
    writeRequest(6, { method: 'PUT', path: '/v1/me/ratings/songs/1', query: {}, body }),
  );
  assert.deepEqual(response, { jsonrpc: '2.0', id: 6, result: { data: [{ id: '1' }] } });
  assert.equal(calls[0]?.url, 'https://api.music.apple.com/v1/me/ratings/songs/1');
  assert.equal(calls[0]?.init.method, 'PUT');
  assert.equal(calls[0]?.init.body, JSON.stringify(body));
  assert.deepEqual(calls[0]?.init.headers, {
    Authorization: 'Bearer dev',
    'Music-User-Token': 'user',
    'Content-Type': 'application/json',
  });
});

void test('api.write / empty response body / should return null', async () => {
  const { fetchFn, calls } = stubFetch(202, '');
  const handlers = createHandlers(fakeMusic(TOKENS), fetchFn);
  const response = await dispatchRequest(
    handlers,
    writeRequest(7, { method: 'POST', path: '/v1/me/favorites', query: { 'ids[songs]': '1' } }),
  );
  assert.deepEqual(response, { jsonrpc: '2.0', id: 7, result: null });
  assert.equal(calls[0]?.init.body, undefined);
});

// Sends each request and checks that all give invalid_params and that no fetch happens.
async function assertAllRejected(requests: readonly (readonly [string, string])[]): Promise<void> {
  const { fetchFn, calls } = stubFetch(200, '{}');
  const handlers = createHandlers(fakeMusic(TOKENS), fetchFn);
  for (const [method, path] of requests) {
    const response = await dispatchRequest(handlers, writeRequest(8, { method, path, query: {} }));
    assert.ok('error' in response && response.error.code === ErrorCode.InvalidParams, `${method} ${path}`);
  }
  assert.equal(calls.length, 0);
}

void test('api.write / each permitted write / should send the request', async () => {
  const { fetchFn, calls } = stubFetch(204, '');
  const handlers = createHandlers(fakeMusic(TOKENS), fetchFn);
  const permitted = [
    ['POST', '/v1/me/library'],
    ['POST', '/v1/me/library/playlists'],
    ['POST', '/v1/me/library/playlists/p.TestPlaylist0001/tracks'],
    ['POST', '/v1/me/favorites'],
    ['PUT', '/v1/me/ratings/songs/123'],
    ['DELETE', '/v1/me/ratings/songs/123'],
    ['DELETE', '/v1/me/ratings/albums/456'],
    ['DELETE', '/v1/me/ratings/playlists/pl.u-abc_1'],
  ];
  for (const [method, path] of permitted) {
    const response = await dispatchRequest(handlers, writeRequest(8, { method, path, query: {} }));
    assert.ok('result' in response, `${method} ${path}`);
  }
  assert.equal(calls.length, permitted.length);
});

void test('api.write / path outside the patterns / should return invalid_params without a request', async () => {
  await assertAllRejected([
    ['POST', '/v1/catalog/it/songs'],
    ['POST', '/v1/me'],
    ['POST', '/v1/me/'],
    ['POST', '/v1/me/library/'],
    ['POST', '/v1/me/library/playlist-folders'],
    ['POST', '/v1/me/../catalog/it'],
    ['POST', '/v1/me/library?x=1'],
    ['DELETE', '/v1/me/ratings/songs'],
    ['DELETE', '/v1/me/ratings/songs/1/extra'],
  ]);
});

void test('api.write / method that the pattern does not permit / should return invalid_params without a request', async () => {
  await assertAllRejected([
    ['PUT', '/v1/me/library'],
    ['PUT', '/v1/me/ratings/albums/1'],
    ['PUT', '/v1/me/ratings/artists/1'],
    ['DELETE', '/v1/me/ratings/artists/1'],
    ['DELETE', '/v1/me/ratings/library-songs/i.1'],
    ['DELETE', '/v1/me/library/playlists/p.1'],
  ]);
});

void test('api.write / ID that can change the URL path / should return invalid_params without a request', async () => {
  const ids = ['', '.', '..', '%2e%2e', '%2e%2e/%2e%2e/%2e%2e/catalog/us', '..\\..\\catalog', 'a\tb', 'a b'];
  await assertAllRejected(ids.map((id) => ['DELETE', `/v1/me/ratings/songs/${id}`] as const));
});

void test('api.write / DELETE on a rating path / should send the request', async () => {
  const { fetchFn, calls } = stubFetch(204, '');
  const handlers = createHandlers(fakeMusic(TOKENS), fetchFn);
  const response = await dispatchRequest(
    handlers,
    writeRequest(10, { method: 'DELETE', path: '/v1/me/ratings/songs/1', query: {} }),
  );
  assert.deepEqual(response, { jsonrpc: '2.0', id: 10, result: null });
  assert.equal(calls[0]?.init.method, 'DELETE');
});

void test('api.write / HTTP error with Apple code / should keep the status and code in the data', async () => {
  const errorBody = '{"errors":[{"code":"40005","title":"Invalid Parameter Value"}]}';
  const { fetchFn } = stubFetch(400, errorBody);
  const handlers = createHandlers(fakeMusic(TOKENS), fetchFn);
  const response = await dispatchRequest(
    handlers,
    writeRequest(11, { method: 'POST', path: '/v1/me/favorites', query: { 'ids[albums]': 'l.x' } }),
  );
  assert.deepEqual(response, {
    jsonrpc: '2.0',
    id: 11,
    error: {
      code: ErrorCode.MusicKitError,
      message: 'api.write failed with HTTP 400',
      data: { status: 400, apple_code: '40005' },
    },
  });
});

void test('api.write / HTTP 401 without error body / should map to not_authorized', async () => {
  const { fetchFn } = stubFetch(401, '');
  const handlers = createHandlers(fakeMusic(TOKENS), fetchFn);
  const response = await dispatchRequest(
    handlers,
    writeRequest(12, { method: 'POST', path: '/v1/me/library', query: {} }),
  );
  assert.ok('error' in response);
  assert.equal(response.error.code, ErrorCode.NotAuthorized);
  assert.deepEqual(response.error.data, { status: 401 });
});
