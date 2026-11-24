// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use serde::Deserialize;

use super::resource::Resource;
use super::resource_map::ResourceMap;
use super::shared::ResourceRef;

/// One titled group of a browse page, for example "Recent releases".
///
/// # Follow-ups
///
/// - Groups of stations are skipped until stations are supported: on the New
///   page "Nuovi episodi radio" and the "Ascolta ora" items of the containers
///   of kind `385`. Videos and curators are skipped the same way.
/// - `Recommendation` (Home) has the same shape and can become a `Group`.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// Title to show to the user. Some groups have none.
    pub title: Option<String>,
    /// How to show the items.
    pub kind: GroupKind,
    /// Items of the group. Never empty, never [`Resource::Unknown`].
    pub items: Vec<Resource>,
}

/// How the UI shows the items of a [`Group`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GroupKind {
    /// A row of tiles: albums, playlists, or a mix.
    TileRow,
    /// A list of songs.
    SongList,
}

impl GroupKind {
    /// Chooses the kind from the items: all songs make a song list.
    fn of(items: &[Resource]) -> Self {
        let all_songs = items
            .iter()
            .all(|item| matches!(item, Resource::Song(_) | Resource::LibrarySong(_)));
        if all_songs {
            Self::SongList
        } else {
            Self::TileRow
        }
    }
}

impl Group {
    /// Builds a group. Returns `None` when there are no items.
    fn new(title: Option<String>, items: Vec<Resource>) -> Option<Self> {
        if items.is_empty() {
            return None;
        }
        Some(Self {
            title,
            kind: GroupKind::of(&items),
            items,
        })
    }
}

/// A response with `format[resources]=map`: references and the resource map.
#[derive(Debug, Deserialize)]
pub(crate) struct MapResponse {
    #[serde(default)]
    data: Vec<ResourceRef>,
    #[serde(default)]
    resources: ResourceMap,
}

impl MapResponse {
    /// Reads the New page: grouping, then tab, then groups. Containers
    /// become one tile row.
    pub(crate) fn new_page_groups(&self) -> Vec<Group> {
        self.data
            .iter()
            .filter_map(|grouping| self.resources.parse::<Element>(grouping))
            .flat_map(|grouping| grouping.relationships.tabs.data)
            .filter_map(|tab| self.resources.parse::<Element>(&tab))
            .flat_map(|tab| tab.relationships.children.data)
            .filter_map(|child| self.resources.parse::<Element>(&child))
            .filter_map(|element| self.group_of(element))
            .collect()
    }

    /// Reads the Made For You page: one flat row of library playlists.
    pub(crate) fn flat_groups(&self) -> Vec<Group> {
        let items = self.resources.resolve(&self.data);
        Group::new(None, items).into_iter().collect()
    }

    /// Builds a group from an element. A container has `children`: the
    /// contents of all children make one row.
    fn group_of(&self, element: Element) -> Option<Group> {
        let Element {
            attributes,
            relationships,
        } = element;
        let references = if relationships.children.data.is_empty() {
            relationships.contents.data
        } else {
            relationships
                .children
                .data
                .iter()
                .filter_map(|child| self.resources.parse::<Element>(child))
                .flat_map(|child| child.relationships.contents.data)
                .collect()
        };
        Group::new(attributes.name, self.resources.resolve(&references))
    }
}

/// An `editorial-elements` or `groupings` object of the New page.
#[derive(Deserialize)]
struct Element {
    #[serde(default)]
    attributes: ElementAttributes,
    #[serde(default)]
    relationships: ElementRelationships,
}

#[derive(Default, Deserialize)]
struct ElementAttributes {
    name: Option<String>,
}

#[derive(Default, Deserialize)]
struct ElementRelationships {
    #[serde(default)]
    tabs: References,
    #[serde(default)]
    children: References,
    #[serde(default)]
    contents: References,
}

#[derive(Default, Deserialize)]
struct References {
    #[serde(default)]
    data: Vec<ResourceRef>,
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests may panic")]
mod tests {
    use serde_json::json;

    use super::*;

    fn response() -> MapResponse {
        serde_json::from_value(json!({
            "data": [{ "id": "g", "type": "groupings" }],
            "resources": {
                "groupings": { "g": { "relationships": { "tabs": { "data": [
                    { "id": "default", "type": "editorial-elements" }] } } } },
                "editorial-elements": {
                    "default": { "relationships": { "children": { "data": [
                        { "id": "songs", "type": "editorial-elements" },
                        { "id": "box", "type": "editorial-elements" },
                        { "id": "radio", "type": "editorial-elements" }] } } },
                    "songs": { "attributes": { "name": "Hits" },
                        "relationships": { "contents": { "data": [
                            { "id": "s1", "type": "songs" }] } } },
                    "box": { "relationships": { "children": { "data": [
                        { "id": "c1", "type": "editorial-elements" }] } } },
                    "c1": { "relationships": { "contents": { "data": [
                        { "id": "a1", "type": "albums" }] } } },
                    "radio": { "attributes": { "name": "Radio" },
                        "relationships": { "contents": { "data": [
                            { "id": "r1", "type": "stations" }] } } }
                },
                "songs": { "s1": { "id": "s1", "type": "songs", "attributes": { "name": "S" } } },
                "albums": { "a1": { "id": "a1", "type": "albums", "attributes": { "name": "A" } } },
                "stations": { "r1": { "id": "r1", "type": "stations" } }
            }
        }))
        .unwrap()
    }

    #[test]
    fn new_page_groups_flatten_containers_and_drop_unsupported_groups() {
        let groups = response().new_page_groups();

        let summary: Vec<_> = groups
            .iter()
            .map(|group| (group.title.as_deref(), group.kind, group.items.len()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (Some("Hits"), GroupKind::SongList, 1),
                (None, GroupKind::TileRow, 1)
            ]
        );
    }

    #[test]
    fn group_kind_is_a_song_list_only_when_all_items_are_songs() {
        let groups = response().new_page_groups();
        let mixed = [groups[0].items[0].clone(), groups[1].items[0].clone()];

        assert_eq!(GroupKind::of(&groups[0].items), GroupKind::SongList);
        assert_eq!(GroupKind::of(&mixed), GroupKind::TileRow);
    }
}
