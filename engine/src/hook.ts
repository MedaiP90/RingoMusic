// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Hook script injected into the page: controls MusicKit.
// All page-specific code is in this file.

import { mapNowPlaying, mapPlaybackState, RateLimiter } from './musickit-mapping.ts';
import type { Notification } from './protocol.ts';

const POLL_INTERVAL_MS = 250;
const POLL_TIMEOUT_MS = 30_000;
// Protocol limit: 4 `player.position` notifications each second.
const POSITION_INTERVAL_MS = 250;
// Error code `musickit_error` from protocol/PROTOCOL.md.
const MUSICKIT_ERROR_CODE = -32005;
const METHOD_NOT_FOUND_CODE = -32601;

interface SelfCheck {
  name: string;
  detail: string;
  passes: (candidate: unknown) => boolean;
}

function hasFunction(candidate: unknown, name: string): boolean {
  return typeof (candidate as Record<string, unknown> | null)?.[name] === 'function';
}

const SELF_CHECKS: SelfCheck[] = [
  {
    name: 'api.music',
    detail: 'MusicKit instance has no api.music function',
    passes: (candidate) => hasFunction((candidate as { api?: unknown }).api, 'music'),
  },
  {
    name: 'addEventListener',
    detail: 'MusicKit instance has no addEventListener function',
    passes: (candidate) => hasFunction(candidate, 'addEventListener'),
  },
];

function getBridge(): RingoBridge {
  const bridge = window.ringoBridge;
  if (!bridge) {
    throw new Error('ringoBridge is not available');
  }
  return bridge;
}

function notify(notification: Notification): void {
  getBridge().send(notification);
}

function tryGetInstance(): MusicKitInstance | undefined {
  try {
    return window.MusicKit?.getInstance();
  } catch {
    // MusicKit exists but is not configured yet.
    return undefined;
  }
}

// Polls until MusicKit gives an instance. Returns `undefined` at the time limit.
async function waitForInstance(): Promise<MusicKitInstance | undefined> {
  const deadline = Date.now() + POLL_TIMEOUT_MS;
  for (;;) {
    const instance = tryGetInstance();
    if (instance) {
      return instance;
    }
    if (Date.now() >= deadline) {
      return undefined;
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
  }
}

function reportIncompatible(check: string, detail: string): void {
  notify({ jsonrpc: '2.0', method: 'engine.incompatible', params: { check, detail } });
}

// Returns `true` when all checks pass. Otherwise it reports the first failed check.
function runSelfChecks(instance: MusicKitInstance): boolean {
  const failed = SELF_CHECKS.find((check) => !check.passes(instance));
  if (failed) {
    reportIncompatible(failed.name, failed.detail);
  }
  return !failed;
}

function subscribe(music: MusicKitInstance): void {
  const positionLimiter = new RateLimiter(POSITION_INTERVAL_MS);

  music.addEventListener('playbackStateDidChange', ({ state }) => {
    const mapped = mapPlaybackState(state);
    if (mapped) {
      notify({ jsonrpc: '2.0', method: 'player.state', params: { state: mapped } });
    }
  });
  music.addEventListener('nowPlayingItemDidChange', ({ item }) => {
    const params = mapNowPlaying(item, music.nowPlayingItemIndex);
    if (params) {
      notify({ jsonrpc: '2.0', method: 'player.nowPlaying', params });
    }
  });
  music.addEventListener('playbackTimeDidChange', ({ currentPlaybackTime }) => {
    if (positionLimiter.tryPass(Date.now())) {
      const position_ms = Math.round(currentPlaybackTime * 1000);
      notify({ jsonrpc: '2.0', method: 'player.position', params: { position_ms } });
    }
  });
  music.addEventListener('authorizationStatusDidChange', () => {
    notify({ jsonrpc: '2.0', method: 'auth.changed', params: { authorized: music.isAuthorized } });
  });
  music.addEventListener('mediaPlaybackError', ({ message }) => {
    notify({
      jsonrpc: '2.0',
      method: 'error',
      params: { code: MUSICKIT_ERROR_CODE, message: message ?? 'playback error', fatal: false },
    });
  });
}

// Prompt 09 adds the real handlers. Until then, each request gets "method not found".
function answerRequestsWithNotFound(): void {
  getBridge().onRequest((request) => {
    const { id, method } = request as { id?: unknown; method?: unknown };
    if (typeof id === 'number') {
      const message = `method not found: ${String(method)}`;
      getBridge().send({ jsonrpc: '2.0', id, error: { code: METHOD_NOT_FOUND_CODE, message } });
    }
  });
}

async function main(): Promise<void> {
  answerRequestsWithNotFound();
  const music = await waitForInstance();
  if (!music) {
    reportIncompatible('musickit.instance', 'MusicKit.getInstance() gave no instance in 30 s');
    return;
  }
  if (!runSelfChecks(music)) {
    return;
  }
  subscribe(music);
  notify({ jsonrpc: '2.0', method: 'engine.ready' });
}

// A page can run the injection twice. The flag stops a second run.
if (!window.__ringoHookInjected) {
  window.__ringoHookInjected = true;
  void main();
}
