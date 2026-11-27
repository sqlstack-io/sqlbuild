//! A finished test's expectations as the child rows under its result row.

use crate::nested_progress::models::NestedChildRow;
use crate::progress::models::ExpectationStep;
use crate::test_command::_helpers::summary::expectation_summary;
use crate::test_command::constants::{ASSERTION_LABEL, ASSERTION_PREFIX, EXPECTED_LABEL};
use crate::test_command::main::test_outcome_status::test_outcome_status;

/// `expected <model>` or `assertion <name>` for each step, with its status and what differed.
#[must_use]
pub fn expectation_child_rows(steps: &[ExpectationStep]) -> Vec<NestedChildRow> {
    steps.iter().map(child_row).collect()
}

fn child_row(step: &ExpectationStep) -> NestedChildRow {
    let (label, name): (&str, &str) = match step.model_name.strip_prefix(ASSERTION_PREFIX) {
        Some(assertion) => (ASSERTION_LABEL, assertion),
        None => (EXPECTED_LABEL, &step.model_name),
    };
    NestedChildRow {
        label: label.to_owned(),
        name: name.to_owned(),
        status_text: test_outcome_status(&step.outcome).to_owned(),
        detail: expectation_summary(step),
    }
}
