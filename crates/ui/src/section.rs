// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Sections of the sidebar.

use gettextrs::gettext;

/// A top-level section. Each section has one row in the sidebar and one
/// navigation stack in the content area.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Section {
    Home,
    Search,
    Playlists,
    Albums,
    Artists,
    Songs,
    Queue,
}

impl Section {
    /// All sections. Each has a page in the content area.
    pub const ALL: [Self; 7] = [
        Self::Home,
        Self::Search,
        Self::Playlists,
        Self::Albums,
        Self::Artists,
        Self::Songs,
        Self::Queue,
    ];

    /// The sections that have a row in the sidebar, in row order. The player
    /// bar opens the queue, so it has no row.
    pub const SIDEBAR: [Self; 6] = [
        Self::Home,
        Self::Search,
        Self::Playlists,
        Self::Albums,
        Self::Artists,
        Self::Songs,
    ];

    /// Returns the section of the sidebar row at `index`.
    pub fn from_row_index(index: i32) -> Option<Self> {
        let index = usize::try_from(index).ok()?;
        Self::SIDEBAR.get(index).copied()
    }

    /// Index of the sidebar row of the section. `None` if it has no row.
    pub fn row_index(self) -> Option<i32> {
        let index = Self::SIDEBAR.iter().position(|section| *section == self)?;
        i32::try_from(index).ok()
    }

    /// Stable name for the stack child and the navigation page tag.
    pub fn id(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Search => "search",
            Self::Playlists => "playlists",
            Self::Albums => "albums",
            Self::Artists => "artists",
            Self::Songs => "songs",
            Self::Queue => "queue",
        }
    }

    /// Translated name of the section.
    pub fn title(self) -> String {
        match self {
            Self::Home => gettext("Home"),
            Self::Search => gettext("Search"),
            Self::Playlists => gettext("Playlists"),
            Self::Albums => gettext("Albums"),
            Self::Artists => gettext("Artists"),
            Self::Songs => gettext("Songs"),
            Self::Queue => gettext("Queue"),
        }
    }

    /// Symbolic icon of the section.
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Home => "user-home-symbolic",
            Self::Search => "system-search-symbolic",
            Self::Playlists => "view-list-bullet-symbolic",
            Self::Albums => "media-optical-symbolic",
            Self::Artists => "avatar-default-symbolic",
            Self::Songs => "audio-x-generic-symbolic",
            Self::Queue => "view-list-ordered-symbolic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_row_index_follows_the_order_of_the_sidebar() {
        for (index, section) in Section::SIDEBAR.into_iter().enumerate() {
            let index = i32::try_from(index).expect("the section count is small");
            assert_eq!(Section::from_row_index(index), Some(section));
        }
    }

    #[test]
    fn row_index_is_the_inverse_of_from_row_index() {
        for section in Section::SIDEBAR {
            let index = section.row_index().expect("the section has a row");
            assert_eq!(Section::from_row_index(index), Some(section));
        }
    }

    #[test]
    fn queue_has_no_sidebar_row() {
        assert_eq!(Section::Queue.row_index(), None);
    }

    #[test]
    fn from_row_index_rejects_values_out_of_range() {
        assert_eq!(Section::from_row_index(-1), None);
        assert_eq!(Section::from_row_index(6), None);
    }
}
