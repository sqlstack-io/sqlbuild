//! The decisions the local runner's JSON reports, one per test or scenario.

use serde_json::Value;

use crate::run_on::_helpers::runner_json::decision;
use crate::run_on::models::RunOnDecision;

/// `{"decisions": [{name, run_on, refusals: [...]}, ...]}` as decisions; malformed JSON is an error.
pub fn read_run_on_decisions(json: &str) -> Result<Vec<RunOnDecision>, String> {
    let document: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let decisions: &Vec<Value> = document
        .get("decisions")
        .and_then(Value::as_array)
        .ok_or_else(|| "run-on decisions have no `decisions` list".to_owned())?;
    Ok(decisions.iter().map(decision).collect())
}
