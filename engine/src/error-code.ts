// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Error codes are in their own file because the hook (page code) cannot import `rpc.ts`.

// Error codes from protocol/PROTOCOL.md.
export const ErrorCode = {
  ParseError: -32700,
  InvalidRequest: -32600,
  MethodNotFound: -32601,
  InvalidParams: -32602,
  InternalError: -32603,
  NotAuthorized: -32001,
  NoSubscription: -32002,
  NotReady: -32003,
  Cancelled: -32004,
  MusicKitError: -32005,
  SandboxFailed: -32006,
} as const;
