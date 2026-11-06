//! Normalize migration metadata and SQL for the persisted migration fingerprint.

use crate::planner_changes::main::version_identity_payload::version_identity_payload;
use crate::planner_changes::models::IdentityPayload;
use crate::planner_migrations::_helpers::proofs::local_names::{
    Rename, local_names, renames, unambiguous,
};
use crate::planner_migrations::constants::{EXCLUDED_CONFIG, QUOTE_DELIMITERS, REF_MARKER};
use crate::planner_migrations::models::ParsedStatement;
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(crate) fn metadata(text: &str) -> Value {
    match version_identity_payload(Some(text), false) {
        IdentityPayload::Absent => Value::Null,
        IdentityPayload::Raw(text) => Value::String(text),
        IdentityPayload::Object(object) => Value::Object(object),
    }
}

pub(crate) fn identity_metadata(text: &str, identities: &BTreeMap<String, String>) -> Value {
    let mut payload = metadata(text);
    if let Some(object) = payload.as_object_mut() {
        object.shift_remove("model_name");
        if let Some(config) = object.get_mut("config").and_then(Value::as_object_mut) {
            config.retain(|key, _| !EXCLUDED_CONFIG.contains(&key.as_str()));
            if let Some(Value::Object(inputs)) = config.get_mut("cursor_inputs") {
                *inputs = std::mem::take(inputs)
                    .into_iter()
                    .map(|(name, value)| (identities.get(&name).cloned().unwrap_or(name), value))
                    .collect();
            }
        }
    }
    payload
}

/// The SQL with every model reference replaced by a placeholder for its rename identity.
pub(crate) fn placeholders(
    sql: &str,
    identities: &BTreeMap<String, String>,
) -> Result<String, String> {
    let marker = Regex::new(REF_MARKER).map_err(|error| error.to_string())?;
    Ok(marker
        .replace_all(sql, |captures: &regex::Captures<'_>| {
            let name = captures
                .get(1)
                .or_else(|| captures.get(2))
                .map_or("", |value| value.as_str());
            let identity = identities.get(name).map_or(name, String::as_str);
            let digest = format!("{:x}", Sha256::digest(identity.as_bytes()));
            format!("__sqb_migration_ref__{}", &digest[..16])
        })
        .into_owned())
}

/// Canonical source text of one parsed statement, or None when its local names are ambiguous.
///
/// The text is the statement's tokens joined by single spaces, so whitespace and comments drop
/// out. CTE names and relation aliases become `__sqb_local_<n>` in order of definition, at the
/// source spans the parser recorded. Tokens with a quote or dollar delimiter keep their spelling;
/// every other token is ASCII-lowercased. No SQL is generated, so any parser that yields the tree
/// with identifier spans and token spans gives the same text.
pub(crate) fn normalized(sql: &str, statement: &ParsedStatement) -> Option<String> {
    let definitions = local_names(&statement.tree);
    if !unambiguous(&statement.tree, &definitions) {
        return None;
    }
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for (name, _) in &definitions {
        let next = names.len() + 1;
        names
            .entry(name.clone())
            .or_insert_with(|| format!("__sqb_local_{next}"));
    }
    let mut ctes: BTreeMap<String, String> = BTreeMap::new();
    for (name, is_cte) in &definitions {
        if *is_cte && let Some(value) = names.get(name) {
            ctes.insert(name.clone(), value.clone());
        }
    }
    let mut found: Vec<Rename> = Vec::new();
    renames(&statement.tree, &names, &ctes, &mut found);
    let mut replacements: BTreeMap<usize, (usize, String)> = BTreeMap::new();
    for (range, replacement) in found {
        let range = range?;
        replacements.insert(range.start, (range.end, replacement));
    }
    let characters: Vec<char> = sql.chars().collect();
    let mut parts: Vec<String> = Vec::with_capacity(statement.tokens.len());
    let mut skip_until: usize = 0;
    let mut used: usize = 0;
    for token in &statement.tokens {
        if token.start < skip_until {
            continue;
        }
        if let Some((end, replacement)) = replacements.get(&token.start) {
            parts.push(replacement.clone());
            skip_until = *end;
            used += 1;
            continue;
        }
        let text: String = characters.get(token.clone())?.iter().collect();
        parts.push(if text.contains(QUOTE_DELIMITERS) {
            text
        } else {
            text.to_ascii_lowercase()
        });
    }
    (used == replacements.len()).then(|| parts.join(" "))
}
