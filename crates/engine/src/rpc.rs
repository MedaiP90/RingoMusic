// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! JSON-RPC connection to the engine over a Unix socket.

mod connection;
mod incoming;
mod pending;

pub use connection::RpcConnection;

#[cfg(test)]
mod tests;
