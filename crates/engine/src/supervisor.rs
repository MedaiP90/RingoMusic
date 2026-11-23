// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Starts the engine process and restarts it after a crash.

mod outputs;
mod process;
mod restart_policy;
mod run;
mod session;
mod socket;

pub(crate) use outputs::Outputs;
#[cfg(test)]
pub(crate) use process::ChildProcess;
pub(crate) use process::{Launcher, SystemLauncher};
pub(crate) use run::Supervisor;
pub(crate) use socket::bind;

#[cfg(test)]
mod tests;
