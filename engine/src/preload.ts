// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Preload script: bridge between the page and the main process.

import { contextBridge, ipcRenderer } from 'electron';

import { CHANNEL_MESSAGE, CHANNEL_REQUEST, isBridgeMessage } from './bridge.ts';

contextBridge.exposeInMainWorld('ringoBridge', {
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
