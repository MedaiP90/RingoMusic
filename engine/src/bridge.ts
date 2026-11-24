// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Shared constants and validation for the page <-> main bridge.

// IPC channel: page to main (notifications and responses).
export const CHANNEL_MESSAGE = 'ringo:message';
// IPC channel: main to page (requests).
export const CHANNEL_REQUEST = 'ringo:request';

// Command line argument that gives the random page bridge name to the preload.
export const BRIDGE_KEY_ARG = '--bridge-key=';

export type BridgeMessage = Record<string, unknown>;

// A bridge message is an object with a string `method` or a numeric `id`.
export function isBridgeMessage(value: unknown): value is BridgeMessage {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const { method, id } = value as BridgeMessage;
  return typeof method === 'string' || typeof id === 'number';
}
