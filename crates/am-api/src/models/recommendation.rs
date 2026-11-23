// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use super::lenient::LenientVec;
use super::resource::Resource;
use super::shared::Relationship;

/// A group of recommended resources with a title, for example "Recently played".
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(from = "RawRecommendation")]
pub struct Recommendation {
    /// ID of the recommendation.
    pub id: String,
    /// Title to show to the user.
    pub title: Option<String>,
    /// Recommended resources.
    pub contents: LenientVec<Resource>,
}

/// Shape of a recommendation in the JSON response.
#[derive(serde::Deserialize)]
struct RawRecommendation {
    id: String,
    attributes: Option<RawAttributes>,
    relationships: Option<RawRelationships>,
}

#[derive(serde::Deserialize)]
struct RawAttributes {
    title: Option<RawTitle>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTitle {
    string_for_display: Option<String>,
}

#[derive(serde::Deserialize)]
struct RawRelationships {
    contents: Option<Relationship<Resource>>,
}

impl From<RawRecommendation> for Recommendation {
    fn from(raw: RawRecommendation) -> Self {
        let title = raw
            .attributes
            .and_then(|attributes| attributes.title)
            .and_then(|title| title.string_for_display);
        let contents = raw
            .relationships
            .and_then(|relationships| relationships.contents)
            .map(|contents| contents.data)
            .unwrap_or_default();
        Self {
            id: raw.id,
            title,
            contents,
        }
    }
}
