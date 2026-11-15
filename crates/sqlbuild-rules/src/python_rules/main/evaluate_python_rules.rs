//! Run the selected Python rules over the request's Python sources.

use std::collections::{BTreeMap, BTreeSet};

use crate::models::{EvaluateRequest, RuleMetadata};
use crate::python_rules::_helpers::engine::execution::selected_rules;
use crate::python_rules::_helpers::engine::layout::layout_faults;
use crate::python_rules::_helpers::engine::passes::evaluated_pass;
use crate::python_rules::_helpers::units::compile_purity::compile_purity_faults;
use crate::python_rules::_helpers::units::orchestration::orchestration_faults;
use crate::python_rules::_helpers::units::project::unit_faults;
use crate::python_rules::_helpers::units::providers::provider_faults;
use crate::python_rules::_helpers::units::python_tests::python_test_faults;
use crate::python_rules::models::{PythonCacheEntry, PythonEvaluation, PythonRuleSpec};

/// Sorted findings of the selected Python rules, reusing `cached` ones made under `fingerprint` that still hold.
pub(crate) fn evaluate_python_rules(
    request: &EvaluateRequest,
    selected: &[&RuleMetadata],
    cached: &BTreeMap<String, PythonCacheEntry>,
    fingerprint: &str,
) -> Result<PythonEvaluation, String> {
    let codes: BTreeSet<&str> = selected.iter().map(|rule| rule.code.as_str()).collect();
    let rules: Vec<&'static PythonRuleSpec> = selected_rules(selected);
    let mut evaluation: PythonEvaluation =
        evaluated_pass(request, (&rules, false), cached, fingerprint)?;
    let tests: PythonEvaluation = evaluated_pass(request, (&rules, true), cached, fingerprint)?;
    evaluation.faults.extend(tests.faults);
    evaluation.entries.extend(tests.entries);
    evaluation.hits += tests.hits;
    evaluation.misses += tests.misses;
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
    evaluation.faults.extend(orchestration_faults(
        &request.python_sources,
        &codes,
        request.python_version,
    ));
    evaluation.faults.extend(python_test_faults(
        &request.python_sources,
        &request.python_test_sources,
        &codes,
    ));
    evaluation
        .faults
        .extend(compile_purity_faults(request, &codes)?);
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
