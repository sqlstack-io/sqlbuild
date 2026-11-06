//! Persisted rename identity of a model.

use crate::planner_migrations::_helpers::proofs::encoding::encode;
use crate::planner_migrations::_helpers::proofs::fingerprint::{
    identity_metadata, normalized, placeholders,
};
use crate::planner_migrations::_helpers::proofs::parse::parse_statement;
use crate::planner_migrations::models::ParsedStatement;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Hash canonical SQL and metadata as migration-fingerprint-v2, parsing with Polyglot.
pub fn migration_fingerprint(
    sql: &str,
    metadata: &str,
    identities: &BTreeMap<String, String>,
    dialect: &str,
) -> Result<Option<String>, String> {
    migration_fingerprint_with(sql, metadata, identities, |text| {
        parse_statement(text, dialect)
    })
}

/// Hash canonical SQL and metadata as migration-fingerprint-v2 with any parser.
///
/// `parse` receives the SQL with model references replaced by placeholders, and returns None when
/// it rejects the SQL. The canonical text is cut from the source by the spans `parse` returns.
pub fn migration_fingerprint_with(
    sql: &str,
    metadata: &str,
    identities: &BTreeMap<String, String>,
    parse: impl FnOnce(&str) -> Result<Option<ParsedStatement>, String>,
) -> Result<Option<String>, String> {
    let text = placeholders(sql, identities)?;
    let Some(statement) = parse(&text)? else {
        return Ok(None);
    };
    let Some(sql) = normalized(&text, &statement) else {
        return Ok(None);
    };
    let payload =
        encode(&json!({"metadata": identity_metadata(metadata, identities), "sql": sql}))?;
    Ok(Some(format!("{:x}", Sha256::digest(payload.as_bytes()))))
}
