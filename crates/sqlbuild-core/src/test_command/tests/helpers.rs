use crate::progress::models::ExpectationStep;
use crate::test_command::models::{TestFailureFacts, TestParameter};

/// A string parameter with its rendered JSON value.
pub(super) fn parameter(name: &str, nullable: bool, rendered: &str) -> TestParameter {
    TestParameter {
        name: name.to_owned(),
        value_type: "string".to_owned(),
        nullable,
        rendered: rendered.to_owned(),
    }
}

/// A local test named `name` with no error, steps or files yet.
pub(super) fn facts(name: &str) -> TestFailureFacts {
    TestFailureFacts {
        name: name.to_owned(),
        kind: "test".to_owned(),
        run_on: "local".to_owned(),
        ..TestFailureFacts::default()
    }
}

/// An expectation on `model_name` with `outcome` and no rows yet.
pub(super) fn step(model_name: &str, outcome: &str) -> ExpectationStep {
    ExpectationStep {
        model_name: model_name.to_owned(),
        outcome: outcome.to_owned(),
        ..ExpectationStep::default()
    }
}
