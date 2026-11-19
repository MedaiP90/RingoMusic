// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Pure mapping from MusicKit values to protocol values. It has no page access.

import type { Notification } from './protocol.ts';

type Params<M extends Notification['method']> = Extract<Notification, { method: M }> extends {
  params: infer P;
}
  ? P
  : never;

export type PlayerState = Params<'player.state'>['state'];
export type NowPlaying = Params<'player.nowPlaying'>;

// MusicKit `PlaybackStates` values. Seeking (6) is not mapped: the state stays as it was.
const STATE_BY_CODE: Record<number, PlayerState> = {
  0: 'stopped',
  1: 'loading',
  2: 'playing',
  3: 'paused',
  4: 'stopped',
  5: 'ended',
  8: 'loading',
  9: 'loading',
  10: 'ended',
};

export function mapPlaybackState(code: number): PlayerState | undefined {
  return STATE_BY_CODE[code];
}

// Returns `undefined` when there is no item or no valid queue index.
export function mapNowPlaying(
  item: MusicKitMediaItem | null | undefined,
  queueIndex: number,
): NowPlaying | undefined {
  if (!item || queueIndex < 0) {
    return undefined;
  }
  const attributes = item.attributes;
  return {
    id: item.id,
    title: attributes?.name ?? item.title ?? '',
    artist: attributes?.artistName ?? item.artistName ?? '',
    album: attributes?.albumName ?? item.albumName ?? '',
    duration_ms: Math.round(attributes?.durationInMillis ?? item.playbackDuration ?? 0),
    // Rust replaces `{w}` and `{h}` (Phase 2).
    artwork_url: attributes?.artwork?.url ?? item.artwork?.url ?? null,
    queue_index: queueIndex,
  };
}

// Lets one call through for each interval. MusicKit fires time events more often than needed.
export class RateLimiter {
  private lastPass = Number.NEGATIVE_INFINITY;

  constructor(private readonly intervalMs: number) {}

  tryPass(now: number): boolean {
    if (now - this.lastPass < this.intervalMs) {
      return false;
    }
    this.lastPass = now;
    return true;
  }
}
