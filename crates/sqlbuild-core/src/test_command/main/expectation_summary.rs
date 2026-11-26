//! The few words a failed expectation's child row gives: `3 rows differ`, or `2 rows`.

use crate::progress::models::ExpectationStep;
use crate::test_command::constants::{ASSERTION_PREFIX, PASS_OUTCOME};

/// How many rows an assertion returned or an expectation got wrong; empty when it passed.
#[must_use]
pub fn expectation_summary(step: &ExpectationStep) -> String {
    if step.outcome == PASS_OUTCOME {
        return String::new();
    }
    let rows = |count: i64| {
        if count == 1 {
            "1 row".to_owned()
        } else {
            format!("{count} rows")
        }
    };
    if step.model_name.starts_with(ASSERTION_PREFIX) {
        return rows(step.actual_row_count);
    }
    let differing: i64 = match (step.missing_row_count, step.unexpected_row_count) {
        (None, None) => step.mismatched_row_count,
        (missing, unexpected) => missing.unwrap_or(0) + unexpected.unwrap_or(0),
    };
    if differing == 0 {
        return String::new();
    }
    if differing == 1 {
        return "1 row differs".to_owned();
    }
    format!("{differing} rows differ")
}
