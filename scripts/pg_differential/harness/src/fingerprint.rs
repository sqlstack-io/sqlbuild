//! Migration fingerprints from SQLBuild's Polyglot parse and from SQLQuack's parse.
//!
//! The fingerprint cuts its canonical text from the source by the spans the parser returns, so
//! the two parsers must give the same fingerprint for every model either of them accepts.

use std::collections::BTreeMap;

use sqlbuild_analysis::planner_migrations::main::migration_fingerprint::{
    migration_fingerprint, migration_fingerprint_with,
};

use crate::compare::Difference;
use crate::engine::Engine;

/// Metadata does not depend on the parser, so both sides hash the same empty object.
const NO_METADATA: &str = "{}";

/// `agree`, `differ` or `not_measured` (neither parser gives a fingerprint).
pub fn compare_fingerprint(engine: Engine, sql: &str, differences: &mut Vec<Difference>) -> String {
    let identities: BTreeMap<String, String> = BTreeMap::new();
    let polyglot = migration_fingerprint(sql, NO_METADATA, &identities, engine.analysis_name());
    let sqlquack = migration_fingerprint_with(sql, NO_METADATA, &identities, |text| {
        engine.fingerprint_statement(text)
    });
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
