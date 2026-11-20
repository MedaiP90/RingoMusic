// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! What a view that loads data shows. The views name their stack pages with it.

/// The state of a view that loads data.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Phase {
    /// The page was not shown yet. It looks like `Loading`.
    Idle,
    Loading,
    Empty,
    Failed,
    Content,
    /// A filter hides all items. Only filtered lists use it.
    NoMatches,
}

impl Phase {
    /// Name of the stack page.
    pub fn page(self) -> &'static str {
        match self {
            Self::Idle | Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Failed => "failed",
            Self::Content => "content",
            Self::NoMatches => "no-matches",
        }
    }
}
