// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { ErrorCode } from './error-code.ts';
import type { Notification } from './protocol.ts';
import { decidePopup, isAllowedPopupUrl, SignIn } from './sign-in.ts';
import type { PageCaller } from './sign-in.ts';

const LOGIN_LOAD = 'load https://music.apple.com/login';
const START_LOAD = 'load https://music.apple.com';
const AUTHORIZED: Notification = {
  jsonrpc: '2.0',
  method: 'auth.changed',
  params: { authorized: true },
};
const READY: Notification = { jsonrpc: '2.0', method: 'engine.ready' };

function nextTick(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

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

void test('run / auth.changed authorized / should hide the window and reload the start page', async () => {
  const harness = new Harness();
  const result = harness.signIn.run();
  await nextTick();
  harness.signIn.handleNotification(AUTHORIZED);
  await nextTick();
  harness.signIn.handleNotification(READY);
  assert.equal(await result, null);
  assert.deepEqual(harness.events, ['auth.status', LOGIN_LOAD, 'show', 'hide', START_LOAD]);
});

void test('run / start page reloading / should wait for engine.ready', async () => {
  const harness = new Harness();
  let settled = false;
  const result = harness.signIn.run().finally(() => {
    settled = true;
  });
  await nextTick();
  harness.signIn.handleNotification(AUTHORIZED);
  await nextTick();
  assert.equal(settled, false);
  harness.signIn.handleNotification(READY);
  await result;
  assert.equal(settled, true);
});

void test('run / engine.ready during login / should not end the login', async () => {
  const harness = new Harness();
  const result = harness.signIn.run();
  await nextTick();
  harness.signIn.handleNotification(READY);
  await nextTick();
  assert.equal(harness.events.includes('hide'), false);
  harness.signIn.handleNotification(AUTHORIZED);
  await nextTick();
  harness.signIn.handleNotification(READY);
  assert.equal(await result, null);
});

void test('run / login page fails to load / should still show the window', async () => {
  const harness = new Harness();
  harness.loadFails = true;
  const result = harness.signIn.run();
  await nextTick();
  harness.signIn.cancel();
  await assert.rejects(result, { code: ErrorCode.Cancelled });
  assert.deepEqual(harness.events.slice(1), [LOGIN_LOAD, 'show', 'hide']);
});

void test('run / window closed / should reject with cancelled', async () => {
  const harness = new Harness();
  const result = harness.signIn.run();
  await nextTick();
  harness.signIn.cancel();
  await assert.rejects(result, { code: ErrorCode.Cancelled });
  assert.equal(harness.events.at(-1), 'hide');
});
