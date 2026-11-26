//! The `sqb build` selectors that load what `sqb load --select` names.

use crate::load_command::models::LoadKinds;

/// Each of `selectors` (every node when empty) narrowed to the loads, loaders and sources it selects; none when the project loads nothing.
#[must_use]
pub fn load_selectors(selectors: &[String], kinds: LoadKinds) -> Vec<String> {
    if !kinds.loads && !kinds.loaders {
        return Vec::new();
    }
    let present: Vec<&str> = [
        (kinds.loaders, "kind:loader"),
        (kinds.loads, "kind:load"),
        (kinds.sources, "kind:source"),
    ]
    .into_iter()
    .filter_map(|(present, kind)| present.then_some(kind))
    .collect();
    if selectors.is_empty() {
        return present.iter().map(|kind| (*kind).to_owned()).collect();
    }
    selectors
        .iter()
        .flat_map(|selector| present.iter().map(move |kind| format!("{selector},{kind}")))
        .collect()
}
