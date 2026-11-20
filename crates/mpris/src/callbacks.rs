// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::fmt;
use std::sync::Arc;

type Callback = Arc<dyn Fn() + Send + Sync>;

/// Actions of the application that D-Bus clients can ask for. The server calls
/// them from a background thread.
#[derive(Clone)]
pub struct Callbacks {
    raise: Callback,
    quit: Callback,
}

impl Callbacks {
    /// Creates the callbacks. `raise` shows the main window. `quit` ends the
    /// application.
    #[must_use]
    pub fn new(
        raise: impl Fn() + Send + Sync + 'static,
        quit: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            raise: Arc::new(raise),
            quit: Arc::new(quit),
        }
    }

    pub(crate) fn raise(&self) {
        (self.raise)();
    }

    pub(crate) fn quit(&self) {
        (self.quit)();
    }
}

impl fmt::Debug for Callbacks {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Callbacks").finish_non_exhaustive()
    }
}
