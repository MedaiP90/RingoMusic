// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { ErrorCode } from './error-code.ts';
import {
  mapError,
  mapNowPlaying,
  mapPlaybackState,
  mapRepeatMode,
  RateLimiter,
} from './musickit-mapping.ts';

void test('mapPlaybackState / known codes / should return the protocol string', () => {
  assert.equal(mapPlaybackState(2), 'playing');
  assert.equal(mapPlaybackState(3), 'paused');
  assert.equal(mapPlaybackState(10), 'ended');
});

void test('mapPlaybackState / seeking code / should return undefined', () => {
  assert.equal(mapPlaybackState(6), undefined);
});

void test('mapNowPlaying / item with attributes / should keep the artwork template', () => {
  const item: MusicKitMediaItem = {
    id: '1',
    attributes: {
      name: 'Song',
      artistName: 'Artist',
      albumName: 'Album',
      durationInMillis: 1000,
      artwork: { url: 'https://x/{w}x{h}.jpg' },
    },
  };
  assert.deepEqual(mapNowPlaying(item, 3), {
    id: '1',
    title: 'Song',
    artist: 'Artist',
    album: 'Album',
    duration_ms: 1000,
    artwork_url: 'https://x/{w}x{h}.jpg',
    queue_index: 3,
  });
});

void test('mapNowPlaying / no item or negative index / should return undefined', () => {
  assert.equal(mapNowPlaying(null, 0), undefined);
  assert.equal(mapNowPlaying({ id: '1' }, -1), undefined);
});

void test('rateLimiter / four calls in 1 s / should pass only calls 250 ms apart', () => {
  const limiter = new RateLimiter(250);
  assert.deepEqual([0, 100, 250, 300].map((now) => limiter.tryPass(now)), [true, false, true, false]);
});

void test('mapRepeatMode / each mode / should return the MusicKit code', () => {
  assert.equal(mapRepeatMode('none'), 0);
  assert.equal(mapRepeatMode('one'), 1);
  assert.equal(mapRepeatMode('all'), 2);
});

void test('mapError / HTTP 401 / should return not_authorized', () => {
  assert.deepEqual(mapError({ status: 401, message: 'no token' }), {
    code: ErrorCode.NotAuthorized,
    message: 'no token',
  });
});

void test('mapError / HTTP 403 in response / should return no_subscription', () => {
  assert.equal(mapError({ response: { status: 403 } }).code, ErrorCode.NoSubscription);
});

void test('mapError / other failure / should return musickit_error with the message', () => {
  assert.deepEqual(mapError(new Error('boom')), { code: ErrorCode.MusicKitError, message: 'boom' });
  assert.deepEqual(mapError({ status: 500 }).code, ErrorCode.MusicKitError);
  assert.equal(mapError('text').message, 'text');
});
