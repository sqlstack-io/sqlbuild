//! One scenario check as an expectation step.

use crate::progress::models::ExpectationStep;
use crate::scenario_command::models::{AssertionRow, ExpectedRow};

/// `error` for a check that could not run, which has no counts, else `fail` or `pass`.
fn outcome(failed: bool, ran: bool) -> String {
    match (failed, ran) {
        (_, false) => "error",
        (true, true) => "fail",
        (false, true) => "pass",
    }
    .to_owned()
}

pub(crate) fn expected_step(expected: &ExpectedRow) -> ExpectationStep {
    ExpectationStep {
        model_name: expected.model_name.clone(),
        outcome: outcome(expected.failed, expected.actual_row_count.is_some()),
        actual_row_count: expected.actual_row_count.unwrap_or(0),
        expected_row_count: expected.expected_row_count.unwrap_or(0),
        mismatched_row_count: expected.mismatched_row_count.unwrap_or(0),
        unexpected_row_count: expected.mismatched_row_count,
        missing_row_count: expected.missing_row_count,
        ..ExpectationStep::default()
    }
}

pub(crate) fn assertion_step(assertion: &AssertionRow) -> ExpectationStep {
    ExpectationStep {
        model_name: format!("assertion {}", assertion.name),
        outcome: outcome(assertion.failed, assertion.failing_row_count.is_some()),
        actual_row_count: assertion.failing_row_count.unwrap_or(0),
        unexpected_samples: assertion
            .first_sample
            .iter()
            .map(|sample| vec![("row".to_owned(), sample.clone())])
            .collect(),
        ..ExpectationStep::default()
    }
}
