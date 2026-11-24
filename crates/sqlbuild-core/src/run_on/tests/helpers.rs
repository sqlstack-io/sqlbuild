use crate::run_on::models::{LocalRefusal, RunOnDecision};

/// A test that runs locally.
pub(super) fn local(name: &str) -> RunOnDecision {
    RunOnDecision {
        name: name.to_owned(),
        run_on: "local".to_owned(),
        refusals: Vec::new(),
    }
}

/// A test SQLQuack refuses at `line` of `stg_orders.sql` for `construct`.
pub(super) fn refused(name: &str, construct: &str, line: i64) -> RunOnDecision {
    RunOnDecision {
        name: name.to_owned(),
        run_on: "warehouse".to_owned(),
        refusals: vec![LocalRefusal {
            category: "a known difference that isn't allowed".to_owned(),
            construct: construct.to_owned(),
            reason: "fn:HASH needs known divergence `hash`".to_owned(),
            code: Some("SQ0008".to_owned()),
            message: Some("HASH gives a different value in DuckDB".to_owned()),
            label: Some("DuckDB computes its own hash, never Snowflake's".to_owned()),
            notes: vec!["Equal inputs still get equal hashes within one run.".to_owned()],
            help: Some("allow it with hash = true under [allow] in a sqlquack.toml".to_owned()),
            path: Some("sql/models/staging/stg_orders.sql".to_owned()),
            line: Some(line),
            column: Some(3),
            width: 12,
            snippet: Some("  HASH(status) AS status_hash".to_owned()),
            snippet_column: None,
            expanded_from: None,
        }],
    }
}

/// A test SQLQuack refuses without a place in any file.
pub(super) fn unplaced(name: &str) -> RunOnDecision {
    RunOnDecision {
        name: name.to_owned(),
        run_on: "warehouse".to_owned(),
        refusals: vec![LocalRefusal {
            category: "missing rule".to_owned(),
            construct: "node:method_call".to_owned(),
            reason: "no rule converts METHOD CALL".to_owned(),
            width: 1,
            ..LocalRefusal::default()
        }],
    }
}
