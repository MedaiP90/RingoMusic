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

export function createHandlers(music: MusicKitInstance): HandlerMap {
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
      await music.setQueue({ songs: items, startWith: start_index });
      // Rust owns the shuffle order (D13). The queue must not start to play.
      music.shuffleMode = SHUFFLE_OFF;
      return null;
    },
    'player.playNext': async ({ items }) => {
      await music.playNext({ songs: items });
      return null;
    },
    'player.playLater': async ({ items }) => {
      await music.playLater({ songs: items });
      return null;
    },
    'player.remove': ({ index }) => {
      music.queue.remove(index);
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
