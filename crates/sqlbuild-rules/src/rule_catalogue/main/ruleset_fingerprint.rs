//! The fingerprint of one resolved ruleset that generated guidance records.

use crate::build_identity::main::native_build_identity::native_build_identity;
use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::models::RulesConfig;
use crate::rule_catalogue::models::FingerprintRule;

/// SHA-256 over the engine build, the selected rules, their sources and the guidance config.
pub fn ruleset_fingerprint(
    rules: &[FingerprintRule],
    config: &RulesConfig,
) -> Result<String, String> {
    let mut payload: Vec<Value> = Vec::with_capacity(rules.len());
    for rule in rules {
        let source_hash: Option<String> = match &rule.source {
            Some(source) => Some(format!(
                "{:x}",
                Sha256::digest(std::fs::read(source).map_err(|error| error.to_string())?)
            )),
            None => None,
        };
        payload.push(json!({
            "code": rule.code,
            "family": rule.family,
            "slug": rule.slug,
            "message": rule.message,
            "remediation": rule.remediation,
            "options": config.rule_options.get(&rule.code).cloned().unwrap_or_else(BTreeMap::new),
            "source_hash": source_hash,
            "implementation_hash": rule.implementation_hash,
        }));
    }
    let rules_payload: Value = json!({
        "engine": native_build_identity(),
        "rules": payload,
        "thresholds": config.thresholds,
        "threshold_overrides": config.threshold_overrides,
        "select_star_allow": config.select_star_allow,
        "domains": config.domains,
        "approved_source_tokens": config.approved_source_tokens,
        "retired_source_tokens": config.retired_source_tokens,
        "sql_tests": config.sql_tests,
        "layout": config.layout,
        "exceptions": config.rule_exceptions,
        "graph_edge_exceptions": config.graph_edge_exceptions,
        "ignores": config.rule_ignores,
    });
    let encoded: String =
        serde_json::to_string(&rules_payload).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(encoded.as_bytes())))
}
