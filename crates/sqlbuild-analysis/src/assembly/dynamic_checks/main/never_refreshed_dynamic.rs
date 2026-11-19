//! Dynamic models with no refresh of their own that no dynamic model reads.

use std::collections::HashSet;

use crate::assembly::dynamic_checks::models::DynamicModel;

/// The first by name; without a reader or its own refresh, the warehouse never refreshes it.
pub fn never_refreshed_dynamic(models: &[DynamicModel]) -> Option<&str> {
    let dynamic: HashSet<&str> = models.iter().map(|model| model.name.as_str()).collect();
    let read: HashSet<&str> = models
        .iter()
        .flat_map(|model| model.reads.iter().map(String::as_str))
        .filter(|name| dynamic.contains(name))
        .collect();
    models
        .iter()
        .filter(|model| !model.has_refresh && !read.contains(model.name.as_str()))
        .map(|model| model.name.as_str())
        .min()
}
