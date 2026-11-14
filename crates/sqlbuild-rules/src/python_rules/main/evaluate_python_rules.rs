//! Run the selected Python rules over the request's Python sources.

use std::collections::BTreeSet;

use crate::models::{EvaluateRequest, Fault, RuleMetadata};
use crate::python_rules::_helpers::contexts::context_settings;
use crate::python_rules::_helpers::execution::{call_faults, rule_calls, selected_rules};
use crate::python_rules::_helpers::layout::layout_faults;
use crate::python_rules::_helpers::placement::python_targets;
use crate::python_rules::_helpers::programs::Programs;
use crate::python_rules::_helpers::project_plane::loaded_project;
use crate::python_rules::models::{CallInputs, PythonRuleSpec, PythonTarget};

/// Faults of the selected Python rules, sorted by path, position and code.
pub(crate) fn evaluate_python_rules(
    request: &EvaluateRequest,
    selected: &[&RuleMetadata],
) -> Result<Vec<Fault>, String> {
    let codes: BTreeSet<&str> = selected.iter().map(|rule| rule.code.as_str()).collect();
    let mut faults: Vec<Fault> = layout_faults(&request.python_sources, &codes);
    faults.extend(fensu_faults(request, selected)?);
    faults.sort_by(|left, right| {
        (&left.path, left.line, left.column, &left.code).cmp(&(
            &right.path,
            right.line,
            right.column,
            &right.code,
        ))
    });
    Ok(faults)
}

/// Faults of the selected rules that Fensu evaluates.
fn fensu_faults(
    request: &EvaluateRequest,
    selected: &[&RuleMetadata],
) -> Result<Vec<Fault>, String> {
    let rules: Vec<&PythonRuleSpec> = selected_rules(selected);
    if rules.is_empty() || request.python_sources.is_empty() {
        return Ok(Vec::new());
    }
    let programs = Programs::new(&request.python_sources, request.python_version);
    let targets: Vec<PythonTarget> = python_targets(&request.python_sources);
    let (plane, entrypoints) = loaded_project(&rules, &targets, &programs, request)?;
    let inputs = CallInputs {
        request,
        settings: context_settings(request, &rules, entrypoints),
        programs: &programs,
    };
    call_faults(rule_calls(&targets, &rules, &inputs)?, &inputs, &plane)
}
