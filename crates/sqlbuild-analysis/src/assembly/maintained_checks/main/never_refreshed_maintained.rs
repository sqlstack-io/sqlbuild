//! Maintained models with no refresh of their own that no maintained model reads.

use std::collections::HashSet;

use crate::assembly::maintained_checks::models::MaintainedModel;

/// The first by name; without a reader or its own refresh, the warehouse never refreshes it.
pub fn never_refreshed_maintained(models: &[MaintainedModel]) -> Option<&str> {
    let maintained: HashSet<&str> = models.iter().map(|model| model.name.as_str()).collect();
    let read: HashSet<&str> = models
        .iter()
        .flat_map(|model| model.reads.iter().map(String::as_str))
        .filter(|name| maintained.contains(name))
        .collect();
    models
        .iter()
        .filter(|model| !model.has_refresh && !read.contains(model.name.as_str()))
        .map(|model| model.name.as_str())
        .min()
}
