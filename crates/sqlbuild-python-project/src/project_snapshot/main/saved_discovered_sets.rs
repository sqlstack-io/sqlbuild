//! The discovered sets a claimed run's snapshot saved, read before the worker compiles.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{PyResult, Python};
use serde_json::Value;
use sqlbuild_orchestration::discovered_sets::models::DiscoveredSet;
use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;
use sqlbuild_orchestration::worker::main::connect_worker::connect_worker;
use sqlbuild_orchestration::worker::main::fetch_execution::fetch_execution;
use sqlbuild_python_base::invocation_phases::constants::DISCOVERED_SET_PHASE;
use sqlbuild_python_base::invocation_phases::models::InvocationPhase;
use sqlbuild_python_base::native_host::main::publish_invocation_phase::publish_invocation_phase;
use uuid::Uuid;

/// The sets the server saved for a claimed run, each published as a phase, so the worker compiles the published nodes.
pub fn saved_discovered_sets(
    py: Python<'_>,
    (server_url, bearer): (&str, &str),
    (run_id, claim_id): (&str, &str),
) -> PyResult<DiscoveredSets> {
    let identity = (
        Uuid::parse_str(run_id).map_err(|error| PyRuntimeError::new_err(error.to_string()))?,
        Uuid::parse_str(claim_id).map_err(|error| PyRuntimeError::new_err(error.to_string()))?,
    );
    let sets: DiscoveredSets = py.detach(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        runtime
            .block_on(async {
                let client = connect_worker(server_url, bearer, identity)?;
                fetch_execution(&client).await
            })
            .map(|execution| execution.discovered)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    })?;
    for (name, set) in &sets {
        publish_invocation_phase(py, &set_phase(name, set))?;
    }
    Ok(sets)
}

/// One saved set as the run used it: its items, tags and when it was discovered; the worker never discovers.
fn set_phase<'a>(name: &str, set: &'a DiscoveredSet) -> InvocationPhase<'a> {
    let tags: Vec<String> = set.result.as_ref().map_or_else(Vec::new, |result| {
        result
            .tags
            .iter()
            .map(|(key, value)| match value {
                Value::String(text) => format!("{key} {text}"),
                other => format!("{key} {other}"),
            })
            .collect()
    });
    let when: Option<String> = set
        .last_good_at
        .or(set.fetched_at)
        .map(|at| format!("discovered {}", at.format("%Y-%m-%d %H:%M UTC")));
    let detail: Vec<String> = tags.into_iter().chain(when).collect();
    InvocationPhase {
        phase: DISCOVERED_SET_PHASE,
        count: set
            .result
            .as_ref()
            .map(|result| i64::try_from(result.items.len()).unwrap_or(i64::MAX)),
        name: Some(name.to_owned()),
        detail: (!detail.is_empty()).then(|| detail.join(" · ")),
        ..InvocationPhase::default()
    }
}
