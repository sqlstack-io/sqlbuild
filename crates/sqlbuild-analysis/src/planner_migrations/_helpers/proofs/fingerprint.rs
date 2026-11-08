//! Normalize migration metadata and SQL without changing the persisted format.

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::planner_changes::main::version_identity_payload::version_identity_payload;
use crate::planner_changes::models::IdentityPayload;
use crate::planner_migrations::_helpers::proofs::ast::strip_formatting;
use crate::planner_migrations::_helpers::proofs::encoding::encode;
use crate::planner_migrations::_helpers::proofs::local_names::{local_names, rewrite, unambiguous};
use crate::planner_migrations::_helpers::proofs::parse::parse;
use crate::planner_migrations::constants::{EXCLUDED_CONFIG, NATIVE_TREE_PREFIX, REF_MARKER};
use polyglot_sql::{Expression, dialects::Dialect};
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

pub(crate) fn normalized(
    sql: &str,
    identities: &BTreeMap<String, String>,
    dialect: &str,
) -> Result<Option<String>, String> {
    let marker = Regex::new(REF_MARKER).map_err(|error| error.to_string())?;
    let placeholder = marker.replace_all(sql, |captures: &regex::Captures<'_>| {
        let name = captures
            .get(1)
            .or_else(|| captures.get(2))
            .map_or("", |value| value.as_str());
        let identity = identities.get(name).map_or(name, String::as_str);
        let digest = format!("{:x}", Sha256::digest(identity.as_bytes()));
        format!("__sqb_migration_ref__{}", &digest[..16])
    });
    let Some(mut parsed) = parse(&placeholder, dialect)? else {
        return Ok(None);
    };
    strip_formatting(&mut parsed);
    let definitions = local_names(&parsed);
    if !unambiguous(&parsed, &definitions) {
        return Ok(None);
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
    rewrite(&mut parsed, &names, &ctes);
    if let AnalysisRoute::Native(_) = analysis_route(Some(dialect)) {
        return encode(&parsed).map(|tree| Some(format!("{NATIVE_TREE_PREFIX}{tree}")));
    }
    let expression: Expression =
        serde_json::from_value(parsed).map_err(|error| error.to_string())?;
    Dialect::get_by_name(dialect)
        .ok_or_else(|| format!("unknown dialect {dialect}"))?
        .generate(&expression)
        .map(Some)
        .map_err(|error| error.to_string())
}
