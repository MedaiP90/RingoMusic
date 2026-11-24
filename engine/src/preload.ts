// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Preload script: bridge between the page and the main process.

import { contextBridge, ipcRenderer } from 'electron';

import { BRIDGE_KEY_ARG, CHANNEL_MESSAGE, CHANNEL_REQUEST, isBridgeMessage } from './bridge.ts';

// Without the key from main, nothing is exposed to the page.
const bridgeKey = process.argv.find((arg) => arg.startsWith(BRIDGE_KEY_ARG))?.slice(BRIDGE_KEY_ARG.length);

if (bridgeKey) {
  exposeBridge(bridgeKey);
}

function exposeBridge(bridgeKey: string): void {
  contextBridge.exposeInMainWorld(bridgeKey, {
    // Sends a notification or a response to main. Invalid messages are dropped.
    send(message: unknown): void {
      if (isBridgeMessage(message)) {
        ipcRenderer.send(CHANNEL_MESSAGE, message);
      }
    },
    // Calls `callback` for each request from main. Invalid requests are dropped.
    onRequest(callback: (request: unknown) => void): void {
      ipcRenderer.on(CHANNEL_REQUEST, (_event, request: unknown) => {
        if (isBridgeMessage(request)) {
          callback(request);
        }
      });
    },
  });
}
