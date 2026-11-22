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

/// One line per sink a block's provider stops: `sink <name> needs <VARS>; every run uses sinks`.
pub(crate) fn sink_lines(blocks: &[SecretBlock]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for block in blocks {
        for sink in &block.sinks {
            lines.push(format!(
                "sink {sink} needs {}; every run uses sinks",
                block.variables.join(", ")
            ));
        }
    }
    lines
}

/// One row per variable of a provider that nodes use: provider, variable, and those nodes and their readers.
pub(crate) fn rows(blocks: &[SecretBlock]) -> Vec<[String; 3]> {
    let mut rows: Vec<[String; 3]> = Vec::new();
    for block in blocks.iter().filter(|block| !block.users.is_empty()) {
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
