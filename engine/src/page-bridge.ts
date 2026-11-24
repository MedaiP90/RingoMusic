// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Main-process side of the bridge: injects the hook, relays page notifications, and calls page methods.

import fs from 'node:fs/promises';
import path from 'node:path';

import { ipcMain } from 'electron';
import type { IpcMainEvent, WebContents } from 'electron';

import { CHANNEL_MESSAGE, CHANNEL_REQUEST, isBridgeMessage } from './bridge.ts';
import type { BridgeMessage } from './bridge.ts';
import { log } from './log.ts';
import type { Notification } from './protocol.ts';
import { ErrorCode, RpcError } from './rpc.ts';

const PAGE_ORIGIN = 'https://music.apple.com';

// The page may send only these notifications. `engine.hello` and `engine.log` belong to main.
const PAGE_NOTIFICATIONS: ReadonlySet<string> = new Set([
  'engine.ready',
  'engine.incompatible',
  'auth.changed',
  'player.state',
  'player.nowPlaying',
  'player.position',
  'error',
]);

interface PendingCall {
  resolve: (result: unknown) => void;
  reject: (error: unknown) => void;
}

export class PageBridge {
  private nextId = 1;
  private ready = false;
  private readonly pending = new Map<number, PendingCall>();

  constructor(
    private readonly webContents: WebContents,
    private readonly notify: (notification: Notification) => void,
    private readonly bridgeKey: string,
  ) {}

  // Starts to listen to the page and injects the hook on each load of the Apple Music page.
  start(): void {
    ipcMain.on(CHANNEL_MESSAGE, (event, message: unknown) => this.receive(event, message));
    this.webContents.on('did-finish-load', () => void this.injectHook());
  }

  // Calls a method in the page. The promise rejects with `RpcError` if the page returns an error,
  // or with `not_ready` if the hook did not send `engine.ready` yet.
  call(method: string, params: unknown): Promise<unknown> {
    return new Promise((resolve, reject) => {
      if (!this.ready) {
        reject(new RpcError(ErrorCode.NotReady, 'engine is not ready'));
        return;
      }
      const id = this.nextId++;
      this.pending.set(id, { resolve, reject });
      log.debug({ id, method }, 'page request');
      this.webContents.send(CHANNEL_REQUEST, { jsonrpc: '2.0', id, method, params });
    });
  }

  private async injectHook(): Promise<void> {
    // A page reload loses the hook and all calls that wait for an answer.
    this.ready = false;
    this.rejectPending();
    if (new URL(this.webContents.getURL()).origin !== PAGE_ORIGIN) {
      return;
    }
    try {
      const script = await fs.readFile(path.join(__dirname, 'hook.js'), 'utf8');
      // The wrapper gives the random bridge name to the hook as `bridgeKey`.
      await this.webContents.executeJavaScript(
        `((bridgeKey) => {\n${script}\n})(${JSON.stringify(this.bridgeKey)})`,
      );
    } catch (error: unknown) {
      log.error({ err: error }, 'hook injection failed');
    }
  }

  private rejectPending(): void {
    for (const call of this.pending.values()) {
      call.reject(new RpcError(ErrorCode.NotReady, 'page reloaded'));
    }
    this.pending.clear();
  }

  private receive(event: IpcMainEvent, message: unknown): void {
    const frame = event.senderFrame;
    const fromPage = frame === this.webContents.mainFrame && new URL(frame.url).origin === PAGE_ORIGIN;
    if (!fromPage || !isBridgeMessage(message)) {
      return;
    }
    if (typeof message['method'] === 'string') {
      this.relayNotification(message, message['method']);
    } else {
      this.settleCall(message);
    }
  }

  private relayNotification(message: BridgeMessage, method: string): void {
    if (PAGE_NOTIFICATIONS.has(method)) {
      if (method === 'engine.ready') {
        this.ready = true;
      }
      // The schema check is in Rust. Here only the method name is checked.
      log.debug({ method, params: message['params'] }, 'page notification');
      this.notify(message as Notification);
    } else {
      log.warn({ method }, 'dropped page notification with unknown method');
    }
  }

  private settleCall(response: BridgeMessage): void {
    const id = response['id'] as number;
    const call = this.pending.get(id);
    if (!call) {
      return;
    }
    this.pending.delete(id);
    log.debug({ id, failed: 'error' in response }, 'page response');
    const error = response['error'] as { code?: unknown; message?: unknown } | undefined;
    if (error) {
      const code = typeof error.code === 'number' ? error.code : ErrorCode.InternalError;
      const text = typeof error.message === 'string' ? error.message : 'page method failed';
      call.reject(new RpcError(code, text));
    } else {
      call.resolve(response['result']);
    }
  }
}
