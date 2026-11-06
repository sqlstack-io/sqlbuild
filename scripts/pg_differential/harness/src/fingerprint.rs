//! Migration fingerprints from SQLBuild's Polyglot parse and from SQLQuack's Postgres parse.
//!
//! The fingerprint cuts its canonical text from the source by the spans the parser returns, so
//! the two parsers must give the same fingerprint for every model either of them accepts.

use std::collections::BTreeMap;

use sqlbuild_analysis::planner_migrations::main::migration_fingerprint::{
    migration_fingerprint, migration_fingerprint_with,
};
use sqlbuild_analysis::planner_migrations::models::ParsedStatement;
use sqlquack_syntax_postgres::scan::main::scan::scan;
use sqlquack_syntax_postgres::scan::models::TokenKind;

use crate::compare::Difference;

const FINGERPRINT_DIALECT: &str = "postgres";
/// Metadata does not depend on the parser, so both sides hash the same empty object.
const NO_METADATA: &str = "{}";

/// `agree`, `differ` or `not_measured` (neither parser gives a fingerprint).
pub fn compare_fingerprint(sql: &str, differences: &mut Vec<Difference>) -> String {
    let identities: BTreeMap<String, String> = BTreeMap::new();
    let polyglot = migration_fingerprint(sql, NO_METADATA, &identities, FINGERPRINT_DIALECT);
    let sqlquack = migration_fingerprint_with(sql, NO_METADATA, &identities, sqlquack_statement);
    if matches!((&polyglot, &sqlquack), (Ok(None), Ok(None))) {
        return "not_measured".to_owned();
    }
    if polyglot == sqlquack {
        return "agree".to_owned();
    }
    differences.push(Difference {
        measure: "fingerprint",
        output: String::new(),
        kind: "fingerprint",
        polyglot: format!("{polyglot:?}"),
        sqlquack: format!("{sqlquack:?}"),
    });
    "differ".to_owned()
}

/// One statement parsed by SQLQuack, or None when it rejects the SQL or keeps it opaque.
fn sqlquack_statement(sql: &str) -> Result<Option<ParsedStatement>, String> {
    let Ok(parsed) = sqlquack_syntax_postgres::parse(sql) else {
        return Ok(None);
    };
    let [statement] = parsed.statements.as_slice() else {
        return Ok(None);
    };
    if !parsed.gaps.is_empty() {
        return Ok(None);
    }
    let tokens = scan(sql);
    if tokens
        .iter()
        .any(|token| matches!(token.kind, TokenKind::Fault(_)))
    {
        return Ok(None);
    }
    Ok(Some(ParsedStatement {
        tree: serde_json::to_value(statement).map_err(|error| error.to_string())?,
        tokens: tokens
            .iter()
            .map(|token| token.span.start..token.span.end)
            .collect(),
    }))
}
