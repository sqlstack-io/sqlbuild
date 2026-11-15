//! Run the selected Python rules over the request's Python sources.

use std::collections::{BTreeMap, BTreeSet};

use crate::models::{EvaluateRequest, RuleMetadata};
use crate::python_rules::_helpers::engine::contexts::context_settings;
use crate::python_rules::_helpers::engine::execution::{
    evaluated_files, file_plans, selected_rules,
};
use crate::python_rules::_helpers::engine::file_cache::project_digest;
use crate::python_rules::_helpers::engine::layout::layout_faults;
use crate::python_rules::_helpers::engine::placement::python_targets;
use crate::python_rules::_helpers::engine::programs::Programs;
use crate::python_rules::_helpers::engine::project_plane::loaded_project;
use crate::python_rules::_helpers::units::project::unit_faults;
use crate::python_rules::_helpers::units::providers::provider_faults;
use crate::python_rules::models::{
    CallInputs, PythonCacheEntry, PythonEvaluation, PythonRuleSpec, PythonTarget,
};

/// Sorted findings of the selected Python rules, reusing `cached` ones made under `fingerprint` that still hold.
pub(crate) fn evaluate_python_rules(
    request: &EvaluateRequest,
    selected: &[&RuleMetadata],
    cached: &BTreeMap<String, PythonCacheEntry>,
    fingerprint: &str,
) -> Result<PythonEvaluation, String> {
    let codes: BTreeSet<&str> = selected.iter().map(|rule| rule.code.as_str()).collect();
    let rules: Vec<&PythonRuleSpec> = selected_rules(selected);
    let mut evaluation = PythonEvaluation::default();
    if !rules.is_empty() && !request.python_sources.is_empty() {
        let programs = Programs::new(&request.python_sources, request.python_version);
        let targets: Vec<PythonTarget> = python_targets(&request.python_sources);
        let (plane, entrypoints) = loaded_project(&rules, &targets, &programs, request)?;
        let inputs = CallInputs {
            request,
            settings: context_settings(request, &rules, entrypoints),
            programs: &programs,
            cached,
            fingerprint,
            project_digest: project_digest(&request.python_sources),
        };
        evaluation = evaluated_files(file_plans(&targets, &rules, &inputs)?, &inputs, &plane)?;
    }
    evaluation
        .faults
        .extend(layout_faults(&request.python_sources, &codes));
    evaluation.faults.extend(unit_faults(
        &request.python_sources,
        &codes,
        request.python_version,
    ));
    evaluation.faults.extend(provider_faults(
        &request.python_sources,
        &codes,
        request.python_version,
    ));
    evaluation.faults.sort_by(|left, right| {
        (&left.path, left.line, left.column, &left.code).cmp(&(
            &right.path,
            right.line,
            right.column,
            &right.code,
        ))
    });
    Ok(evaluation)
}
