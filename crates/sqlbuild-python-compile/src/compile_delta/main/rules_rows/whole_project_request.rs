//! In a delta, the whole project's rules request with the rerun rows taken from `request`.

use crate::compile_delta::_helpers::rules_rows::replaced_tests;
use crate::compile_delta::_helpers::rules_rows::restored;
use pyo3::PyResult;
use sqlbuild_python_base::compile_context::main::delta::delta_unsupported::delta_unsupported;
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use sqlbuild_python_base::compile_context::models::BaseRules;
use sqlbuild_python_base::compile_context::models::PendingRules;
use sqlbuild_rules::engine::main::needs_json_grains::needs_json_grains;
use sqlbuild_rules::models::{EvaluateRequest, Model};
use std::collections::HashMap;

/// In a delta, the whole project's rules request with the rerun rows taken from `request`.
pub fn whole_project_request(request: EvaluateRequest) -> PyResult<EvaluateRequest> {
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
