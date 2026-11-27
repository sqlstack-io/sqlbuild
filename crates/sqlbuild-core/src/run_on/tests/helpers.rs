use serde_json::{Value, json};

use crate::run_on::main::read_run_on_decisions::read_run_on_decisions;
use crate::run_on::main::with_run_on_json::with_run_on_json;
use crate::run_on::models::{LocalRefusal, RunOnDecision, RunOnLayers};

/// A test that runs locally.
pub(super) fn local(name: &str) -> RunOnDecision {
    RunOnDecision {
        relied_on: Vec::new(),
        name: name.to_owned(),
        run_on: "local".to_owned(),
        refusals: Vec::new(),
    }
}

/// A test SQLQuack refuses at `line` of `stg_orders.sql` for `construct`.
pub(super) fn refused(name: &str, construct: &str, line: i64) -> RunOnDecision {
    RunOnDecision {
        relied_on: Vec::new(),
        name: name.to_owned(),
        run_on: "warehouse".to_owned(),
        refusals: vec![hash_refusal(construct, line)],
    }
}

/// SQLQuack's HASH refusal at `line` of `stg_orders.sql`.
fn hash_refusal(construct: &str, line: i64) -> LocalRefusal {
    LocalRefusal {
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
    }
}

/// A test SQLQuack refuses without a place in any file.
pub(super) fn unplaced(name: &str) -> RunOnDecision {
    RunOnDecision {
        relied_on: Vec::new(),
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

/// Run-on layers from the flag, the target and settings.
pub(super) fn layers(
    flag: Option<&str>,
    target: Option<&str>,
    settings: Option<&str>,
) -> RunOnLayers {
    RunOnLayers {
        flag: flag.map(str::to_owned),
        target: target.map(str::to_owned),
        settings: settings.map(str::to_owned),
    }
}

/// A test refused in the output of `@py.status_hash`, placed at the call in the model file.
pub(super) fn refused_in_macro_output(name: &str) -> RunOnDecision {
    RunOnDecision {
        relied_on: Vec::new(),
        name: name.to_owned(),
        run_on: "warehouse".to_owned(),
        refusals: vec![LocalRefusal {
            column: Some(5),
            snippet: Some("  status, HASH(status) AS status_hash".to_owned()),
            snippet_column: Some(11),
            expanded_from: Some(
                "@py.status_hash at sql/models/staging/stg_orders.sql:12:5".to_owned(),
            ),
            ..hash_refusal("fn:HASH", 12)
        }],
    }
}

/// The JSON payload with run-on fields added, or the error as a JSON string.
pub(super) fn run_on_document(decisions: &[RunOnDecision]) -> Value {
    let payload: String = json!({
        "summary": {"total_count": 2},
        "checks": [{"name": "a"}, {"name": "b"}, {"name": "c"}],
    })
    .to_string();
    let local_count: usize = decisions
        .iter()
        .filter(|decision| decision.run_on == "local")
        .count();
    with_run_on_json(
        &payload,
        ("checks", &["name"]),
        decisions,
        (local_count, decisions.len() - local_count),
    )
    .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))
    .unwrap_or_else(Value::String)
}

/// Each refusal's tests, code and line.
pub(super) fn refusal_digest(document: &Value) -> Value {
    Value::Array(
        document["refusals"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|refusal| json!({"tests": refusal["tests"], "code": refusal["code"], "line": refusal["line"]}))
            .collect(),
    )
}

/// Each decision's reason and first refusal width, or `None` when the JSON is rejected.
pub(super) fn read_digest(runner_json: &str) -> Option<Vec<(Option<String>, i64)>> {
    let decisions: Vec<RunOnDecision> = read_run_on_decisions(runner_json).ok()?;
    let mut digest: Vec<(Option<String>, i64)> = Vec::new();
    for decision in &decisions {
        let width: i64 = decision.refusals.first().map_or(0, |refusal| refusal.width);
        digest.push((decision.reason(), width));
    }
    Some(digest)
}
