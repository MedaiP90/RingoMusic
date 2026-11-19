// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Page-side request handlers: one MusicKit call for each protocol method.

import { ErrorCode } from './error-code.ts';
import { mapError, mapRepeatMode } from './musickit-mapping.ts';
import type { PageMethod } from './page-methods.ts';
import type { AuthStatus, ErrorResponse, Request, SuccessResponse } from './protocol.ts';

type ParamsOf<M extends PageMethod> = Extract<Request, { method: M }> extends { params: infer P }
  ? P
  : never;

// A method without params gets a handler without parameters.
export type HandlerMap = {
  [M in PageMethod]: [ParamsOf<M>] extends [never] ? () => unknown : (params: ParamsOf<M>) => unknown;
};

const SHUFFLE_OFF = 0;
const QUEUE_MAX_ATTEMPTS = 3;
const QUEUE_RETRY_DELAY_MS = 1000;

// Catches `mk-007 CONTENT_RESTRICTED` during `setQueue`. Right after the page loads, MusicKit
// can refuse a valid queue. The same call works a moment later. MusicKit reports the refusal as a
// `mediaPlaybackError` event, and `setQueue` still resolves.
export class QueueErrorWatch {
  private watching = false;
  private restricted: string | undefined;

  begin(): void {
    this.watching = true;
    this.restricted = undefined;
  }

  // Stops the watch. Returns the message of a restricted-content error, if one came.
  end(): string | undefined {
    this.watching = false;
    return this.restricted;
  }

  // Returns `true` when the error belongs to the watched `setQueue` call. The caller must then
  // not report it.
  absorb(message: string | undefined): boolean {
    if (this.watching && message !== undefined && /mk-007|CONTENT_RESTRICTED/.test(message)) {
      this.restricted = message;
      return true;
    }
    return false;
  }
}

// ponytail: fixed retry for a MusicKit start-up race. Wait for a real readiness signal if one is found.
async function setQueueWithRetry(
  music: MusicKitInstance,
  watch: QueueErrorWatch,
  options: { songs: string[]; startWith?: number },
  retryDelayMs: number,
): Promise<void> {
  for (let attempt = 1; ; attempt++) {
    watch.begin();
    let restricted: string | undefined;
    try {
      await music.setQueue(options);
    } finally {
      restricted = watch.end();
    }
    if (restricted === undefined) {
      return;
    }
    if (attempt >= QUEUE_MAX_ATTEMPTS) {
      throw new Error(restricted);
    }
    await new Promise((resolve) => setTimeout(resolve, retryDelayMs));
  }
}

export function createHandlers(
  music: MusicKitInstance,
  watch: QueueErrorWatch = new QueueErrorWatch(),
  retryDelayMs: number = QUEUE_RETRY_DELAY_MS,
): HandlerMap {
  return {
    'auth.status': (): AuthStatus => ({
      authorized: music.isAuthorized,
      storefront: music.storefrontId,
    }),
    'auth.signOut': async () => {
      await music.unauthorize();
      return null;
    },
    'player.setQueue': async ({ items, start_index }) => {
      await setQueueWithRetry(music, watch, { songs: items, startWith: start_index }, retryDelayMs);
      // Rust owns the shuffle order (D13). The queue must not start to play.
      music.shuffleMode = SHUFFLE_OFF;
      return null;
    },
    'player.play': async () => {
      await music.play();
      return null;
    },
    'player.pause': async () => {
      await music.pause();
      return null;
    },
    'player.stop': async () => {
      await music.stop();
      return null;
    },
    'player.next': async () => {
      await music.skipToNextItem();
      return null;
    },
    'player.previous': async () => {
      await music.skipToPreviousItem();
      return null;
    },
    'player.seek': async ({ position_ms }) => {
      await music.seekToTime(position_ms / 1000);
      return null;
    },
    'player.setVolume': ({ volume }) => {
      music.volume = volume;
      return null;
    },
    'player.setRepeat': ({ mode }) => {
      music.repeatMode = mapRepeatMode(mode);
      return null;
    },
    'api.get': async ({ path, query }) => {
      const response = await music.api.music(path, query);
      return response.data;
    },
  };
}

export interface PageRequest {
  id: number;
  method: string;
  params?: unknown;
}

function isPageMethod(handlers: HandlerMap, method: string): method is PageMethod {
  return Object.hasOwn(handlers, method);
}

// Runs the handler of the request. A failure becomes an error response, never a throw.
export async function dispatchRequest(
  handlers: HandlerMap,
  { id, method, params }: PageRequest,
): Promise<SuccessResponse | ErrorResponse> {
  if (!isPageMethod(handlers, method)) {
    const message = `method not found: ${method}`;
    return { jsonrpc: '2.0', id, error: { code: ErrorCode.MethodNotFound, message } };
  }
  try {
    const handler = handlers[method] as (params: unknown) => unknown;
    const result = await handler(params);
    return { jsonrpc: '2.0', id, result: result ?? null };
  } catch (error: unknown) {
    return { jsonrpc: '2.0', id, error: mapError(error) };
  }
}
