// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::resource::Resource;
use super::shared::ResourceRef;

/// The `resources` map of a response with `format[resources]=map`.
///
/// The `data` of such a response holds only references. The objects are in
/// this map, by type and then by ID.
#[derive(Debug, Default, serde::Deserialize)]
pub(crate) struct ResourceMap(HashMap<String, HashMap<String, Value>>);

impl ResourceMap {
    /// Returns the object that `reference` points to.
    pub(crate) fn get(&self, reference: &ResourceRef) -> Option<&Value> {
        self.0.get(&reference.kind)?.get(&reference.id)
    }

    /// Parses the object that `reference` points to. Logs and returns `None`
    /// if the object is missing or does not parse.
    pub(crate) fn parse<T: DeserializeOwned>(&self, reference: &ResourceRef) -> Option<T> {
        let Some(value) = self.get(reference) else {
            tracing::warn!(kind = %reference.kind, id = %reference.id, "skipped a reference that is not in the resource map");
            return None;
        };
        match T::deserialize(value) {
            Ok(item) => Some(item),
            Err(error) => {
                tracing::warn!(kind = %reference.kind, id = %reference.id, %error, "skipped a resource that does not parse");
                None
            }
        }
    }

    /// Resolves `references` into resources. Broken, missing, and
    /// [`Resource::Unknown`] items are left out.
    pub(crate) fn resolve(&self, references: &[ResourceRef]) -> Vec<Resource> {
        references
            .iter()
            .filter_map(|reference| self.parse::<Resource>(reference))
            .filter(|resource| *resource != Resource::Unknown)
            .collect()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests may panic")]
mod tests {
    use serde_json::json;

    use super::*;

    fn map() -> ResourceMap {
        serde_json::from_value(json!({
            "albums": {
                "1": { "id": "1", "type": "albums", "attributes": { "name": "One" } },
                "2": { "id": "2", "type": "albums", "attributes": "broken" }
            },
            "stations": { "ra.1": { "id": "ra.1", "type": "stations" } }
        }))
        .unwrap()
    }

    fn reference(kind: &str, id: &str) -> ResourceRef {
        ResourceRef {
            id: id.to_owned(),
            kind: kind.to_owned(),
        }
    }

    #[test]
    fn resolve_keeps_only_valid_known_resources() {
        let references = [
            reference("albums", "1"),
            reference("albums", "2"),
            reference("albums", "3"),
            reference("stations", "ra.1"),
        ];

        let resources = map().resolve(&references);

        assert_eq!(resources.len(), 1);
        assert!(matches!(resources[0], Resource::Album(_)));
    }
}
