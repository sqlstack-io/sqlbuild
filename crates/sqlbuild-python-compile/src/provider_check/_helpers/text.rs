//! Node names and the per-variable rows the provider check's messages show.

use crate::provider_check::models::SecretBlock;

/// A node key's name, such as `raw_payments` for `source:raw_payments`.
pub(crate) fn short(key: &str) -> &str {
    key.split_once(':').map_or(key, |(_, name)| name)
}

/// The names of `keys`, joined with commas.
pub(crate) fn names(keys: &[String]) -> String {
    let names: Vec<&str> = keys.iter().map(|key| short(key)).collect();
    names.join(", ")
}

/// One row per variable: provider, variable, and the nodes that use it and that read those.
pub(crate) fn rows(blocks: &[SecretBlock]) -> Vec<[String; 3]> {
    let mut rows: Vec<[String; 3]> = Vec::new();
    for block in blocks {
        let mut needed_by: String = names(&block.users);
        if !block.readers.is_empty() {
            needed_by.push_str(&format!("; read by {}", names(&block.readers)));
        }
        for variable in &block.variables {
            rows.push([
                block.provider.clone(),
                variable.clone(),
                format!("({needed_by})"),
            ]);
        }
    }
    rows
}
