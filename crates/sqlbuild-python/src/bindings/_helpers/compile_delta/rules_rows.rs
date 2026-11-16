//! A delta compile's built-in rules request: the previous compile's, with the rerun rows fresh.

use std::collections::HashMap;

use pyo3::PyResult;
use sqlbuild_rules::engine::main::needs_json_grains::needs_json_grains;
use sqlbuild_rules::models::{EvaluateRequest, Model, SqlTestFact};

use sqlbuild_python_base::compile_context::main::delta::delta_unsupported::delta_unsupported;
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use sqlbuild_python_base::compile_context::models::BaseRules;
use sqlbuild_python_base::compile_context::models::PendingRules;

/// In a delta, the whole project's rules request with the rerun rows taken from `request`.
pub(crate) fn whole_project_request(request: EvaluateRequest) -> PyResult<EvaluateRequest> {
    let Some(injection) = injection() else {
        return Ok(request);
    };
    if needs_json_grains(&request).unwrap_or(true) {
        return Err(delta_unsupported(
            "SQBRPERFORMANCE102 compares keys and readers across models",
        ));
    }
    let taken: Option<BaseRules> = injection
        .rules
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .and_then(PendingRules::resolved);
    let Some(base) = taken else {
        return Err(delta_unsupported(
            "the previous compile did not run the built-in rules",
        ));
    };
    if !request.initial_findings.is_empty() || !base.request.initial_findings.is_empty() {
        return Err(delta_unsupported(
            "the rules request carries initial findings",
        ));
    }
    let mut whole: EvaluateRequest = restored(base);
    let mut fresh: HashMap<String, Model> = request
        .models
        .into_iter()
        .map(|model| (model.name.clone(), model))
        .collect();
    for model in &mut whole.models {
        if let Some(replacement) = fresh.remove(&model.name) {
            *model = replacement;
        }
    }
    if !fresh.is_empty() {
        return Err(delta_unsupported(
            "a rerun model is new to the rules request",
        ));
    }
    whole.sql_tests = replaced_tests(&whole.sql_tests, request.sql_tests)?;
    Ok(whole)
}

/// The retained request with each row's payload digest restored.
fn restored(base: BaseRules) -> EvaluateRequest {
    let mut request: EvaluateRequest = base.request;
    for (model, digest) in request.models.iter_mut().zip(base.digests) {
        model.payload_digest = Some(digest);
        model.stripped = true;
    }
    request
}

/// `base` with the facts of every file `fresh` covers replaced, in place and in order.
fn replaced_tests(base: &[SqlTestFact], fresh: Vec<SqlTestFact>) -> PyResult<Vec<SqlTestFact>> {
    let mut by_file: HashMap<String, std::vec::IntoIter<SqlTestFact>> = HashMap::new();
    let mut grouped: HashMap<String, Vec<SqlTestFact>> = HashMap::new();
    for fact in fresh {
        grouped
            .entry(fact.source_path.clone())
            .or_default()
            .push(fact);
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for fact in base {
        *counts.entry(fact.source_path.as_str()).or_default() += 1;
    }
    for (path, facts) in grouped {
        if counts.get(path.as_str()).copied().unwrap_or_default() != facts.len() {
            return Err(delta_unsupported(
                "a rerun SQL test file changed its test count",
            ));
        }
        by_file.insert(path, facts.into_iter());
    }
    Ok(base
        .iter()
        .map(|fact| {
            by_file
                .get_mut(&fact.source_path)
                .and_then(Iterator::next)
                .unwrap_or_else(|| fact.clone())
        })
        .collect())
}
