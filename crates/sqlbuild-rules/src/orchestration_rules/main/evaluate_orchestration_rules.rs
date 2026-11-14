//! Findings of the selected orchestration rules over the request's orchestration facts.

use std::collections::BTreeSet;

use sqlbuild_analysis::graph::main::named_selector_atoms::named_selector_atoms;

use crate::models::{EvaluateRequest, Fault, RuleMetadata};
use crate::orchestration_rules::constants::{
    DEFAULT_MAX_NAMED_SELECTORS, MAX_NAMED_SELECTORS_OPTION, NAMED_SELECTOR_LIMIT_CODE,
};
use crate::orchestration_rules::models::OrchestrationSelection;

/// A finding for each selection that names more nodes than its limit, or cannot be counted.
pub(crate) fn evaluate_orchestration_rules(
    request: &EvaluateRequest,
    rule: &RuleMetadata,
) -> Vec<Fault> {
    let limit: u32 = request
        .config
        .rule_option_u32(NAMED_SELECTOR_LIMIT_CODE, MAX_NAMED_SELECTORS_OPTION)
        .unwrap_or(DEFAULT_MAX_NAMED_SELECTORS);
    let run_names: BTreeSet<&str> = request
        .orchestration
        .run_names
        .iter()
        .map(String::as_str)
        .collect();
    request
        .orchestration
        .selections
        .iter()
        .filter_map(|selection| selection_fault(rule, selection, (limit, &run_names)))
        .collect()
}

fn selection_fault(
    rule: &RuleMetadata,
    selection: &OrchestrationSelection,
    (limit, run_names): (u32, &BTreeSet<&str>),
) -> Option<Fault> {
    let OrchestrationSelection { owner, name, .. } = selection;
    if let Some(what) = &selection.unresolved {
        return Some(fault(
            rule,
            selection,
            format!(
                "{owner} '{name}' has a condition the rule cannot resolve: {what}, so the nodes it selects cannot be counted"
            ),
            "Add an exact rule exception for this file with a reason.".to_owned(),
        ));
    }
    let Some(text) = &selection.selection else {
        return Some(fault(
            rule,
            selection,
            format!(
                "{owner} '{name}' has a condition select that is not a string literal, so the nodes it names cannot be counted"
            ),
            "Write the select as a quoted string. If it must be computed, add an exact rule exception for this file with a reason.".to_owned(),
        ));
    };
    let named: BTreeSet<String> = named_selector_atoms(text)
        .into_iter()
        .filter(|atom| !run_names.contains(atom.as_str()))
        .collect();
    let count: usize = named.len();
    (count > limit as usize).then(|| {
        fault(
            rule,
            selection,
            format!("{owner} '{name}' names {count} nodes in its selection (limit {limit})"),
            format!(
                "If these nodes are one pipeline, select its folder or a tag. If the {owner} deliberately targets exactly these nodes, add an exact rule exception for this file with a reason."
            ),
        )
    })
}

fn fault(
    rule: &RuleMetadata,
    selection: &OrchestrationSelection,
    message: String,
    remediation: String,
) -> Fault {
    Fault {
        unevaluated: false,
        code: rule.code.clone(),
        path: selection.path.clone(),
        line: selection.line,
        column: selection.column,
        message,
        remediation,
    }
}
