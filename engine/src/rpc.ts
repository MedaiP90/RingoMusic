// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// JSON-RPC client for the Rust socket: NDJSON framing, handshake, and dispatcher.

import net from 'node:net';

import { ErrorCode } from './error-code.ts';
import { LineFramer } from './framer.ts';
import { log } from './log.ts';
import type { ErrorResponse, Notification, SuccessResponse } from './protocol.ts';

export const PROTOCOL_VERSION = 1;

export { ErrorCode };

// A handler throws this error to send a specific error code to Rust.
export class RpcError extends Error {
  constructor(
    readonly code: number,
    message: string,
  ) {
    super(message);
    this.name = 'RpcError';
  }
}

export type Handler = (params: unknown) => Promise<unknown>;

export interface EngineIdentity {
  engineVersion: string;
  electron: string;
}

interface IncomingRequest {
  id: number;
  method: string;
  params: unknown;
}

type ErrorId = number | null;

export class RpcClient {
  private readonly handlers = new Map<string, Handler>();
  private readonly framer = new LineFramer();

  private constructor(private readonly socket: net.Socket) {}

  // Connects, sends `engine.hello`, and calls `onClose` when the socket closes or fails.
  static connect(path: string, identity: EngineIdentity, onClose: () => void): Promise<RpcClient> {
    return new Promise((resolve, reject) => {
      const socket = net.connect(path);
      socket.setEncoding('utf8');
      const client = new RpcClient(socket);
      socket.once('connect', () => {
        socket.off('error', reject);
        socket.on('data', (chunk: string) => client.receive(chunk));
        socket.once('close', onClose);
        socket.on('error', () => socket.destroy());
        client.notify({
          jsonrpc: '2.0',
          method: 'engine.hello',
          params: {
            proto: PROTOCOL_VERSION,
            engine_version: identity.engineVersion,
            electron: identity.electron,
          },
        });
        resolve(client);
      });
      socket.once('error', reject);
    });
  }

  register(method: string, handler: Handler): void {
    this.handlers.set(method, handler);
  }

  notify(notification: Notification): void {
    this.send(notification);
  }

  // Resolves when the socket has written all queued data. Writes finish in order, so the
  // callback of an empty write runs after every earlier write.
  flush(): Promise<void> {
    if (this.socket.destroyed) {
      return Promise.resolve();
    }
    return new Promise((resolve) => {
      this.socket.write('', () => resolve());
    });
  }

  close(): void {
    this.socket.end();
  }

  private send(message: Notification | SuccessResponse | ErrorResponse): void {
    if (!this.socket.destroyed) {
      this.socket.write(`${JSON.stringify(message)}\n`);
    }
  }

  private receive(chunk: string): void {
    for (const line of this.framer.push(chunk)) {
      void this.handleLine(line);
    }
  }

  private async handleLine(line: string): Promise<void> {
    let message: unknown;
    try {
      message = JSON.parse(line);
    } catch {
      this.sendError(null, ErrorCode.ParseError, 'cannot parse message as JSON');
      return;
    }
    const request = parseRequest(message);
    if (!request) {
      this.sendError(readId(message), ErrorCode.InvalidRequest, 'message is not a valid request');
      return;
    }
    await this.dispatch(request);
  }

  private async dispatch({ id, method, params }: IncomingRequest): Promise<void> {
    const handler = this.handlers.get(method);
    if (!handler) {
      this.sendError(id, ErrorCode.MethodNotFound, `method not found: ${method}`);
      return;
    }
    try {
      const result = await handler(params);
      this.send({ jsonrpc: '2.0', id, result: result ?? null });
    } catch (error: unknown) {
      if (error instanceof RpcError) {
        this.sendError(id, error.code, error.message);
      } else {
        log.error({ method, err: error }, 'handler failed');
        this.sendError(id, ErrorCode.InternalError, 'internal error in handler');
      }
    }
  }

  private sendError(id: ErrorId, code: number, message: string): void {
    this.send({ jsonrpc: '2.0', id, error: { code, message } });
  }
}

function readId(message: unknown): ErrorId {
  if (typeof message === 'object' && message !== null && 'id' in message) {
    return typeof message.id === 'number' ? message.id : null;
  }
  return null;
}

function parseRequest(message: unknown): IncomingRequest | undefined {
  if (typeof message !== 'object' || message === null) {
    return undefined;
  }
  const { id, method } = message as Record<string, unknown>;
  if (typeof id !== 'number' || typeof method !== 'string') {
    return undefined;
  }
  return { id, method, params: (message as Record<string, unknown>)['params'] };
}
