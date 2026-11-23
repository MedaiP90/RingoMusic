// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::ops::Deref;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// A list that skips the elements that do not parse.
///
/// Each element is parsed on its own. A broken element is logged with
/// `tracing::warn!` and left out. The other elements stay in the list.
#[derive(Clone, Debug, PartialEq)]
pub struct LenientVec<T>(Vec<T>);

impl<T> LenientVec<T> {
    /// Returns the elements as a vector.
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<T> Default for LenientVec<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> Deref for LenientVec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<T> IntoIterator for LenientVec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for LenientVec<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let values = Vec::<Value>::deserialize(deserializer)?;
        let items = values.iter().filter_map(parse_element).collect();
        Ok(Self(items))
    }
}

/// Parses one element. Logs the reason and returns `None` when it fails.
fn parse_element<T: DeserializeOwned>(value: &Value) -> Option<T> {
    match T::deserialize(value) {
        Ok(item) => Some(item),
        Err(error) => {
            let resource_type = value
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let id = value.get("id").and_then(Value::as_str).unwrap_or("none");
            tracing::warn!(resource_type, id, %error, "skipped a resource that does not parse");
            None
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests may panic")]
mod tests {
    use super::*;

    #[test]
    fn skips_elements_that_do_not_parse() {
        let list: LenientVec<u32> = serde_json::from_str(r#"[1, "two", 3, null]"#).unwrap();

        assert_eq!(list.into_vec(), vec![1, 3]);
    }

    #[test]
    fn fails_when_the_value_is_not_a_list() {
        let result = serde_json::from_str::<LenientVec<u32>>(r#"{"a": 1}"#);

        assert!(result.is_err());
    }
}
