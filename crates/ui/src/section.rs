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
    New,
    Playlists,
    Albums,
    Artists,
    Songs,
    MadeForYou,
}

impl Section {
    /// All sections, in the order of the sidebar rows.
    pub const ALL: [Self; 8] = [
        Self::Home,
        Self::Search,
        Self::New,
        Self::Playlists,
        Self::Albums,
        Self::Artists,
        Self::Songs,
        Self::MadeForYou,
    ];

    /// Returns the section of the sidebar row at `index`.
    pub fn from_row_index(index: i32) -> Option<Self> {
        let index = usize::try_from(index).ok()?;
        Self::ALL.get(index).copied()
    }

    /// Index of the sidebar row of the section.
    pub fn row_index(self) -> i32 {
        let index = Self::ALL.iter().position(|section| *section == self);
        index
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(0)
    }

    /// Stable name for the stack child and the navigation page tag.
    pub fn id(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Search => "search",
            Self::New => "new",
            Self::Playlists => "playlists",
            Self::Albums => "albums",
            Self::Artists => "artists",
            Self::Songs => "songs",
            Self::MadeForYou => "made-for-you",
        }
    }

    /// Translated name of the section.
    pub fn title(self) -> String {
        match self {
            Self::Home => gettext("Home"),
            Self::Search => gettext("Search"),
            Self::New => gettext("New"),
            Self::Playlists => gettext("Playlists"),
            Self::Albums => gettext("Albums"),
            Self::Artists => gettext("Artists"),
            Self::Songs => gettext("Songs"),
            Self::MadeForYou => gettext("Made For You"),
        }
    }

    /// Symbolic icon of the section.
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Home => "user-home-symbolic",
            Self::Search => "system-search-symbolic",
            Self::New => "starred-symbolic",
            Self::Playlists => "view-list-bullet-symbolic",
            Self::Albums => "media-optical-symbolic",
            Self::Artists => "avatar-default-symbolic",
            Self::Songs => "audio-x-generic-symbolic",
            Self::MadeForYou => "emblem-favorite-symbolic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_row_index_follows_the_order_of_all() {
        for (index, section) in Section::ALL.into_iter().enumerate() {
            let index = i32::try_from(index).expect("the section count is small");
            assert_eq!(Section::from_row_index(index), Some(section));
        }
    }

    #[test]
    fn row_index_is_the_inverse_of_from_row_index() {
        for section in Section::ALL {
            assert_eq!(Section::from_row_index(section.row_index()), Some(section));
        }
    }

    #[test]
    fn from_row_index_rejects_values_out_of_range() {
        assert_eq!(Section::from_row_index(-1), None);
        assert_eq!(Section::from_row_index(8), None);
    }
}
