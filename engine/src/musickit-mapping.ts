// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Pure mapping from MusicKit values to protocol values. It has no page access.

import { ErrorCode } from './error-code.ts';
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

// Finds the album ID of an item: its container if it is an album, else its album link.
// Returns `null` when MusicKit has no such data.
function findAlbumId(item: MusicKitMediaItem): string | null {
  const container = item.container;
  if (container?.id && container.type?.endsWith('albums')) {
    return container.id;
  }
  return item.relationships?.albums?.data?.[0]?.id ?? null;
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
    album_id: findAlbumId(item),
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

export type RepeatMode = 'none' | 'one' | 'all';

// MusicKit `PlaybackRepeatMode` values.
const REPEAT_MODE_CODES: Record<RepeatMode, number> = { none: 0, one: 1, all: 2 };

export function mapRepeatMode(mode: RepeatMode): number {
  return REPEAT_MODE_CODES[mode];
}

export interface MappedError {
  code: number;
  message: string;
}

const CODE_BY_HTTP_STATUS: Record<number, number> = {
  401: ErrorCode.NotAuthorized,
  403: ErrorCode.NoSubscription,
};

function readStatus(error: unknown): number | undefined {
  const candidate = error as { status?: unknown; response?: { status?: unknown } } | null;
  const status = candidate?.status ?? candidate?.response?.status;
  return typeof status === 'number' ? status : undefined;
}

function readMessage(error: unknown): string {
  const message = (error as { message?: unknown } | null)?.message;
  return typeof message === 'string' ? message : String(error);
}

// Maps a MusicKit failure to a protocol error. HTTP 401 and 403 have their own codes.
export function mapError(error: unknown): MappedError {
  const status = readStatus(error);
  const code = (status === undefined ? undefined : CODE_BY_HTTP_STATUS[status]) ?? ErrorCode.MusicKitError;
  return { code, message: readMessage(error) };
}
