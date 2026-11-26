// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import net from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

import { LineFramer } from './framer.ts';
import { ErrorCode, PROTOCOL_VERSION, RpcClient, RpcError } from './rpc.ts';

interface Harness {
  client: RpcClient;
  lines: string[];
  peer: net.Socket;
  waitForLines(count: number): Promise<void>;
  cleanup(): void;
}

// Starts a local server on a temporary socket and connects an `RpcClient` to it.
async function startHarness(): Promise<Harness> {
  const directory = mkdtempSync(path.join(tmpdir(), 'ringo-rpc-'));
  const socketPath = path.join(directory, 'engine.sock');
  const lines: string[] = [];
  const framer = new LineFramer();
  const server = net.createServer();
  const accepted = new Promise<net.Socket>((resolve) => server.once('connection', resolve));
  await new Promise<void>((resolve) => server.listen(socketPath, resolve));
  const client = await RpcClient.connect(
    socketPath,
    { engineVersion: '0.0.0-test', electron: '0.0.0' },
    () => {},
  );
  const peer = await accepted;
  peer.setEncoding('utf8');
  peer.on('data', (chunk: string) => lines.push(...framer.push(chunk)));
  return {
    client,
    lines,
    peer,
    waitForLines: async (count) => {
      while (lines.length < count) {
        await new Promise((resolve) => setImmediate(resolve));
      }
    },
    cleanup: () => {
      client.close();
      peer.destroy();
      server.close();
      rmSync(directory, { recursive: true });
    },
  };
}

void test('rpc / after connect / should send engine.hello as the first line', async () => {
  const directory = mkdtempSync(path.join(tmpdir(), 'ringo-rpc-'));
  const socketPath = path.join(directory, 'engine.sock');
  const server = net.createServer();
  const firstChunk = new Promise<string>((resolve) =>
    server.once('connection', (socket) => socket.once('data', (data) => resolve(String(data)))),
  );
  await new Promise<void>((resolve) => server.listen(socketPath, resolve));
  const client = await RpcClient.connect(
    socketPath,
    { engineVersion: '1.2.3', electron: '9.9.9' },
    () => {},
  );
  const first = JSON.parse((await firstChunk).split('\n')[0] ?? '') as unknown;
  assert.deepEqual(first, {
    jsonrpc: '2.0',
    method: 'engine.hello',
    params: { proto: PROTOCOL_VERSION, engine_version: '1.2.3', electron: '9.9.9' },
  });
  client.close();
  server.close();
  rmSync(directory, { recursive: true });
});

void test('rpc / unknown method / should answer with error -32601', async () => {
  const harness = await startHarness();
  harness.peer.write('{"jsonrpc":"2.0","id":7,"method":"nope"}\n');
  await harness.waitForLines(2);
  const response = JSON.parse(harness.lines[1] ?? '') as { id: number; error: { code: number } };
  assert.equal(response.id, 7);
  assert.equal(response.error.code, ErrorCode.MethodNotFound);
  harness.cleanup();
});

void test('rpc / known method / should answer with the handler result', async () => {
  const harness = await startHarness();
  harness.client.register('player.seek', (params) => Promise.resolve({ echo: params }));
  harness.peer.write('{"jsonrpc":"2.0","id":1,"method":"player.seek","params":{"position_ms":5}}\n');
  await harness.waitForLines(2);
  assert.deepEqual(JSON.parse(harness.lines[1] ?? ''), {
    jsonrpc: '2.0',
    id: 1,
    result: { echo: { position_ms: 5 } },
  });
  harness.cleanup();
});

void test('rpc / handler throws RpcError / should answer with its code', async () => {
  const harness = await startHarness();
  harness.client.register('auth.signIn', () =>
    Promise.reject(new RpcError(ErrorCode.Cancelled, 'user cancelled')),
  );
  harness.peer.write('{"jsonrpc":"2.0","id":2,"method":"auth.signIn"}\n');
  await harness.waitForLines(2);
  const response = JSON.parse(harness.lines[1] ?? '') as { error: { code: number } };
  assert.equal(response.error.code, ErrorCode.Cancelled);
  harness.cleanup();
});

void test('rpc / invalid JSON line / should answer with parse error and null id', async () => {
  const harness = await startHarness();
  harness.peer.write('not json\n');
  await harness.waitForLines(2);
  const response = JSON.parse(harness.lines[1] ?? '') as { id: null; error: { code: number } };
  assert.equal(response.id, null);
  assert.equal(response.error.code, ErrorCode.ParseError);
  harness.cleanup();
});
