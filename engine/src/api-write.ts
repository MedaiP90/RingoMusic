// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Library writes to the Apple Music API. `music.api.music` always sends a GET, so writes use `fetch`.
// The request body and the headers are never logged: they can hold user data and tokens.

import type { ApiWriteRequest } from './protocol.ts';

const API_ORIGIN = 'https://api.music.apple.com';
const WRITE_PREFIX = '/v1/me/';

// Documented `DELETE` rating types. Source: plan/04_library-management/apple-music-writes.md.
// Keep this list equal to `DELETE_PATTERNS` in crates/engine/src/protocol/to_engine.rs.
const DELETE_RATING_TYPES: ReadonlySet<string> = new Set([
  'songs',
  'albums',
  'playlists',
  'music-videos',
  'stations',
  'library-songs',
  'library-albums',
  'library-playlists',
  'library-music-videos',
]);
const DELETE_RATING_PREFIX = '/v1/me/ratings/';

// A request value that the engine does not permit. It maps to `invalid_params`.
export class InvalidParamsError extends Error {}

// An HTTP failure of a write. `status` and `appleCode` go to the log in the error `data`.
export class WriteHttpError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly appleCode: string | undefined,
  ) {
    super(message);
  }
}

// What the write needs from MusicKit and from the page.
export interface WriteContext {
  developerToken: string;
  musicUserToken: string;
  fetch: typeof fetch;
}

function isDocumentedDelete(path: string): boolean {
  if (!path.startsWith(DELETE_RATING_PREFIX)) {
    return false;
  }
  const [type, id, ...extra] = path.slice(DELETE_RATING_PREFIX.length).split('/');
  return DELETE_RATING_TYPES.has(type ?? '') && Boolean(id) && extra.length === 0;
}

// Second check of the rules that Rust applies before the call.
export function validateWrite({ method, path }: ApiWriteRequest): void {
  if (!path.startsWith(WRITE_PREFIX)) {
    throw new InvalidParamsError('api.write path must start with /v1/me/');
  }
  if (/[?#]/.test(path)) {
    throw new InvalidParamsError('api.write path must not hold a query or fragment');
  }
  const segments = path.slice(WRITE_PREFIX.length).split('/');
  if (segments.some((segment) => ['', '.', '..'].includes(segment))) {
    throw new InvalidParamsError('api.write path has an empty or relative segment');
  }
  if (method === 'DELETE' && !isDocumentedDelete(path)) {
    throw new InvalidParamsError('api.write DELETE is not allowed for this path');
  }
}

// Returns the Apple error code of an error body, if it has one.
function readAppleCode(text: string): string | undefined {
  try {
    const body = JSON.parse(text) as { errors?: { code?: unknown }[] } | null;
    const code = body?.errors?.[0]?.code;
    return typeof code === 'string' ? code : undefined;
  } catch {
    return undefined;
  }
}

// Sends a validated write. Returns the response JSON, or `null` for an empty body.
export async function sendWrite(context: WriteContext, request: ApiWriteRequest): Promise<unknown> {
  validateWrite(request);
  const url = new URL(request.path, API_ORIGIN);
  for (const [key, value] of Object.entries(request.query)) {
    url.searchParams.append(key, value);
  }
  const headers: Record<string, string> = {
    Authorization: `Bearer ${context.developerToken}`,
    'Music-User-Token': context.musicUserToken,
  };
  if (request.body !== undefined) {
    headers['Content-Type'] = 'application/json';
  }
  const response = await context.fetch(url, {
    method: request.method,
    headers,
    body: request.body === undefined ? undefined : JSON.stringify(request.body),
  });
  // A 202 or 204 response has an empty body.
  const text = await response.text();
  if (!response.ok) {
    throw new WriteHttpError(
      `api.write failed with HTTP ${response.status}`,
      response.status,
      readAppleCode(text),
    );
  }
  return text === '' ? null : (JSON.parse(text) as unknown);
}
