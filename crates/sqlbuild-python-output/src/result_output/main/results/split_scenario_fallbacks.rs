//! Local scenario replays that failed where the warehouse would not, placed as the run mode says.

use crate::result_output::types::ScenarioResults;
use pyo3::prelude::{PyAnyMethods, PyResult};
use sqlbuild_core::run_on::main::apply_local_refusals::apply_local_refusals;
use sqlbuild_core::run_on::main::read_local_refusal::read_local_refusal;
use sqlbuild_core::run_on::models::{LocalRefusal, RunOn, RunOnDecision};

/// The results to report; in `auto` a scenario a guard or engine error refused is not one: it reruns.
pub fn split_scenario_fallbacks<'py>(
    ran: ScenarioResults<'py>,
    (decisions, mode): (&mut [RunOnDecision], RunOn),
) -> PyResult<ScenarioResults<'py>> {
    let mut results: ScenarioResults<'py> = Vec::new();
    let mut refused: Vec<(String, LocalRefusal)> = Vec::new();
    for result in ran {
        let Some(fields) = &result.local_refusal else {
            results.push(result);
            continue;
        };
        let (name, refusal) = read_local_refusal(fields.extract()?);
        if mode != RunOn::Auto {
            results.push(result);
        }
        refused.push((name, refusal));
    }
    apply_local_refusals(decisions, refused, mode);
    Ok(results)
}
