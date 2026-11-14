use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde_json::Value;
use tempfile::TempDir;

use crate::engine::_helpers::memo::SpeculativeOutcome;
use crate::engine::main::answer_speculative_rows::answer_speculative_rows;
use crate::engine::main::evaluate_rows::evaluate_rows;
use crate::engine::main::start_speculative_rows::start_speculative_rows;
use crate::engine::tests::helpers::{
    SPECULATIVE_CACHE_FILE, SPECULATIVE_PYTHON_CACHE_FILE, answer_after_stop,
    changed_orders_models, discard_after_running, drop_at_once, evaluation_summary, orders_models,
    speculative_rows_request, written_rules_files,
};
use crate::engine::tests::test_types::{
    SpeculativeOutcomeTestCase, SpeculativeRowsTestCase, SpeculativeStopTestCase,
};
use crate::models::SpeculativeAnswer;

#[test]
fn given_speculative_rows_when_answering_then_result_and_writes_match_ordinary_evaluation()
-> Result<(), String> {
    let test_cases = [
        SpeculativeRowsTestCase {
            description: "identical cached request uses the speculation",
            cache_enabled: true,
            changed_query: None,
            expected_used: true,
        },
        SpeculativeRowsTestCase {
            description: "identical uncached request uses the speculation",
            cache_enabled: false,
            changed_query: None,
            expected_used: true,
        },
        SpeculativeRowsTestCase {
            description: "a model changed after the speculation started is evaluated again",
            cache_enabled: true,
            changed_query: Some("SELECT order_id FROM warehouse.raw.orders"),
            expected_used: false,
        },
    ];
    for test_case in &test_cases {
        let speculated_dir = TempDir::new().map_err(|error| error.to_string())?;
        let ordinary_dir = TempDir::new().map_err(|error| error.to_string())?;
        let project_dir: &Path = speculated_dir.path();
        let actual_models: Vec<Value> = changed_orders_models(test_case.changed_query);

        let speculative = start_speculative_rows(speculative_rows_request(
            project_dir,
            test_case.cache_enabled,
            &orders_models(),
        )?)
        .map_err(|error| error.to_string())?;
        let answered = answer_speculative_rows(
            speculative,
            speculative_rows_request(project_dir, test_case.cache_enabled, &actual_models)?,
            &AtomicBool::new(false),
        );
        let expected = evaluate_rows(speculative_rows_request(
            ordinary_dir.path(),
            test_case.cache_enabled,
            &actual_models,
        )?)
        .map_err(|_| "ordinary evaluation failed".to_owned())?;

        let SpeculativeAnswer::Answered { evaluation, used } = answered else {
            return Err(format!("{}: answer stopped", test_case.description));
        };
        let evaluation = evaluation.map_err(|_| "speculative answer failed".to_owned())?;
        let cache = |dir: &Path| {
            [SPECULATIVE_CACHE_FILE, SPECULATIVE_PYTHON_CACHE_FILE]
                .map(|file| std::fs::read(dir.join(file)).ok())
        };
        assert_eq!(used, test_case.expected_used, "{}", test_case.description);
        assert_eq!(
            evaluation_summary(&evaluation),
            evaluation_summary(&expected),
            "{}",
            test_case.description
        );
        assert_eq!(
            cache(project_dir),
            cache(ordinary_dir.path()),
            "{}",
            test_case.description
        );
        assert_eq!(
            written_rules_files(project_dir).len(),
            written_rules_files(ordinary_dir.path()).len(),
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_speculative_rows_when_discarded_or_stopped_then_nothing_is_written() -> Result<(), String>
{
    let test_cases = [
        SpeculativeStopTestCase {
            description: "discarding after it ran a while",
            end: discard_after_running,
            expected_stopped: false,
        },
        SpeculativeStopTestCase {
            description: "answering after the rules run was cancelled",
            end: answer_after_stop,
            expected_stopped: true,
        },
        SpeculativeStopTestCase {
            description: "dropping it at once",
            end: drop_at_once,
            expected_stopped: false,
        },
    ];
    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let speculative = start_speculative_rows(speculative_rows_request(
            project_dir.path(),
            true,
            &orders_models(),
        )?)
        .map_err(|error| error.to_string())?;

        let stopped: bool = (test_case.end)(speculative, project_dir.path())?;

        assert_eq!(
            stopped, test_case.expected_stopped,
            "{}",
            test_case.description
        );
        assert_eq!(
            written_rules_files(project_dir.path()),
            Vec::<PathBuf>::new(),
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_speculative_outcome_when_checked_then_only_failed_evaluations_are_retried() {
    let test_cases = [
        SpeculativeOutcomeTestCase {
            description: "memo reuse is kept",
            outcome: || SpeculativeOutcome::Reused("{}".to_owned()),
            expected_clean: true,
        },
        SpeculativeOutcomeTestCase {
            description: "evaluated response is kept",
            outcome: || SpeculativeOutcome::Evaluated {
                bucket: None,
                response: Ok("{}".to_owned()),
                memo: None,
            },
            expected_clean: true,
        },
        SpeculativeOutcomeTestCase {
            description: "failed evaluation is retried by the real request",
            outcome: || SpeculativeOutcome::Evaluated {
                bucket: None,
                response: Err("worker pool unavailable".to_owned()),
                memo: None,
            },
            expected_clean: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            (test_case.outcome)().evaluated_cleanly(),
            test_case.expected_clean,
            "{}",
            test_case.description
        );
    }
}
