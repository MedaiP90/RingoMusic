// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::fmt;

/// ID of a song in the Apple Music catalog or library.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ItemId(String);

impl ItemId {
    /// Creates an ID from its text form, for example `1440935467` or `i.abc123`.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the text form of the ID.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the text form of the ID and consumes the value.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
