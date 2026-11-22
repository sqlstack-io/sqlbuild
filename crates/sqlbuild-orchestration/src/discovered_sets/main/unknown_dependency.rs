//! The first project node a set's items depend on that the project doesn't have.

use crate::discovered_sets::models::{DiscoveredResult, ItemDependency};

/// Why `result` can't be a set of this project: an item depends on a node `known` rejects.
pub fn unknown_dependency(
    result: &DiscoveredResult,
    known: &impl Fn(&ItemDependency) -> bool,
) -> Option<String> {
    for item in &result.items {
        for dependency in &item.depends_on {
            let (kind, name, missing): (&str, &str, &str) = match dependency {
                ItemDependency::Item(_) => continue,
                ItemDependency::Model(name) => ("model", name, "have"),
                ItemDependency::Source(name) => ("source", name, "have"),
                ItemDependency::Seed(name) => ("seed", name, "have"),
                ItemDependency::Job(set) => ("job", &set.name, "declare"),
                ItemDependency::Tag(_) => continue,
            };
            if !known(dependency) {
                return Some(format!(
                    "item '{}' depends on {kind} '{name}', which the project does not {missing}",
                    item.id
                ));
            }
        }
    }
    None
}
