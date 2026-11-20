// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use super::lenient::LenientVec;

/// Artwork of a resource. The `url` is a template: use `artwork_url` to fill it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct Artwork {
    /// URL template with `{w}`, `{h}`, and `{f}` placeholders.
    pub url: String,
    /// Width of the original image in pixels.
    pub width: Option<u32>,
    /// Height of the original image in pixels.
    pub height: Option<u32>,
}

/// Data that MusicKit needs to play a resource.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayParams {
    /// ID to give to the player.
    pub id: String,
    /// Kind of the resource, for example `song` or `album`.
    pub kind: String,
    /// Catalog ID of a library item, when Apple gives one.
    pub catalog_id: Option<String>,
}

/// Text that describes a resource, for example a playlist.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct Description {
    /// Full text. Apple sends plain text.
    pub standard: Option<String>,
}

/// Content rating of a resource.
#[derive(Copy, Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentRating {
    /// The content has explicit language.
    Explicit,
    /// The content is a clean version.
    Clean,
    /// Apple sent a rating that this app does not know.
    #[serde(other)]
    Other,
}

/// A page of resources, with the link to the next page.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(bound = "T: serde::de::DeserializeOwned")]
pub struct ApiPage<T> {
    /// Resources of this page.
    #[serde(default)]
    pub data: LenientVec<T>,
    /// Path of the next page. `None` on the last page.
    pub next: Option<String>,
}

/// Resources that a resource links to.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(bound = "T: serde::de::DeserializeOwned")]
pub struct Relationship<T> {
    /// Related resources. Can be empty if the request did not include them.
    #[serde(default)]
    pub data: LenientVec<T>,
    /// Path of the next page of related resources.
    pub next: Option<String>,
}

/// Reference to a resource, without its attributes.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct ResourceRef {
    /// ID of the resource.
    pub id: String,
    /// Type of the resource, for example `albums`.
    #[serde(rename = "type")]
    pub kind: String,
}
