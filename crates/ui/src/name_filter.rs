// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! The name filter of a list. The entry sets the term. The list items read it.

use std::cell::RefCell;
use std::rc::Rc;

/// A search term that is shared between the entry and the filter of a list.
/// Clones share the same term.
#[derive(Clone, Debug, Default)]
pub struct NameFilter {
    term: Rc<RefCell<String>>,
}

impl NameFilter {
    /// Sets the term. Case and the spaces around it do not matter.
    pub fn set(&self, term: &str) {
        *self.term.borrow_mut() = term.trim().to_lowercase();
    }

    /// Tells if the term is not empty.
    pub fn is_active(&self) -> bool {
        !self.term.borrow().is_empty()
    }

    /// Tells if one of `names` contains the term. An empty term matches all.
    pub fn matches(&self, names: &[&str]) -> bool {
        let term = self.term.borrow();
        term.is_empty()
            || names
                .iter()
                .any(|name| name.to_lowercase().contains(&*term))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(term: &str) -> NameFilter {
        let filter = NameFilter::default();
        filter.set(term);
        filter
    }

    #[test]
    fn matches_everything_without_a_term() {
        assert!(filter("  ").matches(&["Creep"]));
        assert!(!filter("  ").is_active());
    }

    #[test]
    fn matches_ignores_case_and_surrounding_spaces() {
        assert!(filter(" CREE ").matches(&["Creep"]));
    }

    #[test]
    fn matches_any_of_the_names() {
        assert!(filter("radio").matches(&["Creep", "Radiohead"]));
    }

    #[test]
    fn matches_rejects_a_name_without_the_term() {
        assert!(!filter("xyz").matches(&["Creep", "Radiohead"]));
    }

    #[test]
    fn clones_share_the_term() {
        let filter = NameFilter::default();
        let copy = filter.clone();
        filter.set("a");
        assert!(copy.is_active());
    }
}
