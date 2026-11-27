//! A scenario's expected outputs and assertions as the expectation steps a test reports.

use crate::progress::models::ExpectationStep;
use crate::scenario_command::_helpers::steps::{assertion_step, expected_step};
use crate::scenario_command::models::ScenarioResultRow;

/// Each expected output, whose extra rows are its mismatched ones, then each assertion.
#[must_use]
pub fn scenario_steps(row: &ScenarioResultRow) -> Vec<ExpectationStep> {
    row.expected
        .iter()
        .map(expected_step)
        .chain(row.assertions.iter().map(assertion_step))
        .collect()
}
