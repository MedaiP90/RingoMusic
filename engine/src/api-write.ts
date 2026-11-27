// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Library writes to the Apple Music API. `music.api.music` always sends a GET, so writes use `fetch`.
// The request body and the headers are never logged: they can hold user data and tokens.

import type { ApiWriteRequest } from './protocol.ts';

const API_ORIGIN = 'https://api.music.apple.com';

// The only write requests that RingoMusic sends. Each one is documented by Apple
// (plan/04_library-management/apple-music-writes.md). In a pattern, `{id}` is one ID segment and
// `a|b` is one of the literal segments `a` and `b`.
// Keep this list equal to `WRITE_PATTERNS` in crates/engine/src/protocol/to_engine.rs.
const WRITE_PATTERNS: readonly (readonly [ApiWriteRequest['method'], string])[] = [
  ['POST', '/v1/me/library'],
  ['POST', '/v1/me/library/playlists'],
  ['POST', '/v1/me/library/playlists/{id}/tracks'],
  ['POST', '/v1/me/favorites'],
  ['PUT', '/v1/me/ratings/songs/{id}'],
  ['DELETE', '/v1/me/ratings/songs|albums|playlists/{id}'],
];
const ID_PLACEHOLDER = '{id}';
// ASCII letters, digits, `.`, `_`, and `-`. Other characters can change the path in `new URL()`.
const ID_SEGMENT = /^[A-Za-z0-9._-]+$/;

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

function isIdSegment(segment: string): boolean {
  return segment !== '.' && segment !== '..' && ID_SEGMENT.test(segment);
}

function matchesSegment(segment: string, expected: string): boolean {
  if (expected === ID_PLACEHOLDER) {
    return isIdSegment(segment);
  }
  return expected.split('|').includes(segment);
}

function matchesPattern(path: string, pattern: string): boolean {
  const pathSegments = path.split('/');
  const patternSegments = pattern.split('/');
  return (
    pathSegments.length === patternSegments.length &&
    pathSegments.every((segment, index) => matchesSegment(segment, patternSegments[index] ?? ''))
  );
}

// Second check of the rules that Rust applies before the call.
export function validateWrite({ method, path }: ApiWriteRequest): void {
  const isPermitted = WRITE_PATTERNS.some(
    ([patternMethod, pattern]) => patternMethod === method && matchesPattern(path, pattern),
  );
  if (!isPermitted) {
    throw new InvalidParamsError('api.write is not allowed for this method and path');
  }
}

// Builds the request URL. Fails if `new URL()` changes the path (an encoded dot, a backslash, a
// tab), so the request always goes to the path that `validateWrite` checked.
function buildUrl({ path, query }: ApiWriteRequest): URL {
  const url = new URL(path, API_ORIGIN);
  if (url.pathname !== path) {
    throw new InvalidParamsError('api.write path changes when the URL is built');
  }
  for (const [key, value] of Object.entries(query)) {
    url.searchParams.append(key, value);
  }
  return url;
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
  const url = buildUrl(request);
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
