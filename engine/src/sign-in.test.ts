// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { ErrorCode } from './error-code.ts';
import { decidePopup, isAllowedPopupUrl, SignIn } from './sign-in.ts';
import type { PageCaller } from './sign-in.ts';

void test('isAllowedPopupUrl / apple https hosts / should allow', () => {
  assert.ok(isAllowedPopupUrl('https://idmsa.apple.com/login'));
  assert.ok(isAllowedPopupUrl('https://apple.com/'));
});

void test('isAllowedPopupUrl / other URLs / should deny', () => {
  assert.equal(isAllowedPopupUrl('http://idmsa.apple.com/'), false);
  assert.equal(isAllowedPopupUrl('https://evilapple.com/'), false);
  assert.equal(isAllowedPopupUrl('https://apple.com.evil.test/'), false);
  assert.equal(isAllowedPopupUrl('not a url'), false);
});

void test('decidePopup / denied URL / should deny', () => {
  assert.deepEqual(decidePopup('https://example.com', 'p'), { action: 'deny' });
});

class Harness {
  readonly events: string[] = [];
  authorized = false;
  loadFails = false;
  readonly window = {
    loadURL: (url: string) => {
      this.events.push(`load ${url}`);
      return this.loadFails ? Promise.reject(new Error('load failed')) : Promise.resolve();
    },
    show: () => this.events.push('show'),
    hide: () => this.events.push('hide'),
  };
  readonly page: PageCaller = {
    call: (method) => {
      this.events.push(method);
      return method === 'auth.status'
        ? Promise.resolve({ authorized: this.authorized, storefront: 'us' })
        : new Promise(() => {});
    },
  };
  readonly signIn = new SignIn(this.window, this.page);
}

void test('run / already authorized / should not show the window', async () => {
  const harness = new Harness();
  harness.authorized = true;
  assert.equal(await harness.signIn.run(), null);
  assert.deepEqual(harness.events, ['auth.status']);
});

void test('run / auth.changed authorized / should resolve and hide the window', async () => {
  const harness = new Harness();
  const result = harness.signIn.run();
  await new Promise((resolve) => setImmediate(resolve));
  harness.signIn.handleNotification({
    jsonrpc: '2.0',
    method: 'auth.changed',
    params: { authorized: true },
  });
  assert.equal(await result, null);
  assert.deepEqual(harness.events, [
    'auth.status',
    'load https://music.apple.com/login',
    'show',
    'hide',
  ]);
});

void test('run / login page fails to load / should still show the window', async () => {
  const harness = new Harness();
  harness.loadFails = true;
  const result = harness.signIn.run();
  await new Promise((resolve) => setImmediate(resolve));
  harness.signIn.cancel();
  await assert.rejects(result, { code: ErrorCode.Cancelled });
  assert.deepEqual(harness.events.slice(1), ['load https://music.apple.com/login', 'show', 'hide']);
});

void test('run / window closed / should reject with cancelled', async () => {
  const harness = new Harness();
  const result = harness.signIn.run();
  await new Promise((resolve) => setImmediate(resolve));
  harness.signIn.cancel();
  await assert.rejects(result, { code: ErrorCode.Cancelled });
  assert.equal(harness.events.at(-1), 'hide');
});
