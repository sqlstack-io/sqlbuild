//! A project's `roles.toml` becomes the snapshot `roles` document, its selectors resolved per snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::authorization::_helpers::custom_roles::decode_roles;
use crate::credentials::models::RolesFile;

/// The `roles` document and one warning per grant whose selectors match no node in this snapshot.
pub fn compile_roles(
    text: &str,
    resolve: impl Fn(&str) -> Result<BTreeSet<String>, String>,
) -> Result<(Value, Vec<String>), String> {
    let file: RolesFile = toml::from_str(text).map_err(|error| format!("roles.toml: {error}"))?;
    let mut roles = file.roles;
    let mut warnings: Vec<String> = Vec::new();
    for (key, role) in &mut roles {
        for (index, grant) in role.grants.iter_mut().enumerate() {
            let mut resolved: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for selector in &grant.nodes {
                let nodes = resolve(selector).map_err(|error| {
                    format!("roles.toml: role {key} selector '{selector}': {error}")
                })?;
                resolved.insert(selector.clone(), nodes);
            }
            if resolved.values().all(BTreeSet::is_empty) {
                warnings.push(format!(
                    "role {key} grant {} selects no nodes ({}); it grants nothing in this snapshot",
                    index + 1,
                    grant.nodes.iter().cloned().collect::<Vec<_>>().join(" ")
                ));
            }
            grant.resolved_nodes = resolved;
        }
    }
    let document = serde_json::to_value(&roles).map_err(|error| error.to_string())?;
    decode_roles(Some(&document)).map_err(|error| format!("roles.toml: {error}"))?;
    Ok((document, warnings))
}
