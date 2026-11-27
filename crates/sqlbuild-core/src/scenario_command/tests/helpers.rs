use std::path::{Path, PathBuf};

use crate::nested_progress::models::NestedChildRow;
use crate::scenario_command::errors::ScenarioCommandError;
use crate::scenario_command::models::{
    AssertionRow, CaptureRelationRow, CodedText, ExpectationSql, ExpectedRow, NamespaceCandidates,
    RetainedRelation, RuntimeArtifact, ScenarioFile, ScenarioNamespace, ScenarioResultRow,
};

pub(super) const PROJECT_DIR: &str = "/work/orders_project";

pub(super) fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

pub(super) fn scenario_files() -> Vec<ScenarioFile> {
    [
        ("orders_paid", "tests/scenarios/orders/paid.sql"),
        ("orders_refund", "tests/scenarios/orders/refund.sql"),
        ("customers_new", "tests/scenarios/customers_new.sql"),
    ]
    .iter()
    .map(|(name, relative_path)| ScenarioFile {
        name: (*name).to_owned(),
        file_path: Path::new(PROJECT_DIR).join(relative_path),
        relative_path: PathBuf::from(relative_path),
    })
    .collect()
}

pub(super) fn coded(code: Option<&str>, message: &str, help: Option<&str>) -> Option<CodedText> {
    Some(CodedText {
        code: code.map(str::to_owned),
        message: message.to_owned(),
        help: help.map(str::to_owned),
    })
}

pub(super) fn artifact_pairs(artifacts: Vec<RuntimeArtifact>) -> Vec<(PathBuf, String)> {
    artifacts
        .into_iter()
        .map(|artifact| (artifact.relative_path, artifact.sql))
        .collect()
}

pub(super) fn expected_pairs(expected: &[(PathBuf, &str)]) -> Vec<(PathBuf, String)> {
    expected
        .iter()
        .map(|(path, sql)| (path.clone(), (*sql).to_owned()))
        .collect()
}

pub(super) fn retained(kind: &str, logical_name: &str, physical_name: &str) -> RetainedRelation {
    RetainedRelation {
        kind: kind.to_owned(),
        logical_name: logical_name.to_owned(),
        physical_name: physical_name.to_owned(),
    }
}

pub(super) fn failing_result_row() -> ScenarioResultRow {
    ScenarioResultRow {
        name: "orders_paid".to_owned(),
        status: "FAIL".to_owned(),
        duration_ms: Some(1234.5),
        error: coded(
            Some("X506"),
            "expected comparison failed\nsecond line",
            Some("Compare the CTE."),
        ),
        retained: true,
        retained_relations: Some(vec![
            retained("source", "raw_orders", "main.__sqb_abc__source__raw_orders"),
            retained("model", "fact_orders", "main.__sqb_abc__model__fact_orders"),
        ]),
        expected: vec![
            ExpectedRow {
                model_name: "fact_orders".to_owned(),
                failed: true,
                actual_row_count: Some(3),
                expected_row_count: Some(2),
                mismatched_row_count: Some(2),
                missing_row_count: Some(1),
                error: coded(Some("X506"), "mismatch", Some("look")),
            },
            ExpectedRow {
                model_name: "dim_customers".to_owned(),
                actual_row_count: Some(1),
                expected_row_count: Some(1),
                ..ExpectedRow::default()
            },
        ],
        assertions: vec![
            AssertionRow {
                name: "no_negative".to_owned(),
                failed: true,
                failing_row_count: Some(1),
                first_sample: Some("(1, 'a', None)".to_owned()),
                error: None,
            },
            AssertionRow {
                name: "few".to_owned(),
                failed: true,
                failing_row_count: Some(3),
                first_sample: None,
                error: coded(None, "plain error", None),
            },
            AssertionRow {
                name: "ok".to_owned(),
                failing_row_count: Some(0),
                ..AssertionRow::default()
            },
        ],
        ..ScenarioResultRow::default()
    }
}

pub(super) fn relation(
    kind: &str,
    logical_name: &str,
    (passed, row_count, byte_count): (bool, i64, i64),
) -> CaptureRelationRow {
    CaptureRelationRow {
        kind: kind.to_owned(),
        logical_name: logical_name.to_owned(),
        passed,
        row_count,
        byte_count,
    }
}

pub(super) fn statements(values: &[&str]) -> Vec<String> {
    strings(values)
}

pub(super) fn expectation_sql() -> ExpectationSql {
    ExpectationSql {
        expected: vec![(
            "order_totals".to_owned(),
            "SELECT 1 EXCEPT SELECT 2".to_owned(),
        )],
        assertions: vec![(
            "no_negative".to_owned(),
            "SELECT * FROM t WHERE x < 0".to_owned(),
        )],
    }
}

pub(super) const INVALID_NAMESPACE: &str =
    "scenario.run_namespace must be 1–128 characters: ASCII letters, digits, '-', '_' or '.'";

pub(super) fn namespace(
    value: &str,
    source: &str,
) -> Result<ScenarioNamespace, ScenarioCommandError> {
    Ok(ScenarioNamespace {
        value: Some(value.to_owned()),
        source: source.to_owned(),
    })
}

pub(super) fn invalid_namespace() -> Result<ScenarioNamespace, ScenarioCommandError> {
    Err(ScenarioCommandError {
        code: "C458",
        message: INVALID_NAMESPACE.to_owned(),
        help: None,
    })
}

pub(super) fn candidates(values: [Option<&str>; 4]) -> NamespaceCandidates {
    let [cli, env, local_config, project_config] = values.map(|value| value.map(str::to_owned));
    NamespaceCandidates {
        cli,
        env,
        local_config,
        project_config,
    }
}

pub(super) fn selected_names(
    files: &[ScenarioFile],
    selected: Result<Vec<usize>, ScenarioCommandError>,
) -> Result<Vec<String>, (&'static str, String)> {
    selected
        .map(|indices| {
            indices
                .into_iter()
                .map(|index| files[index].name.clone())
                .collect()
        })
        .map_err(|error| (error.code, error.message))
}

/// A child row with `label`, `name`, `status` and `detail`.
pub(super) fn child(label: &str, name: &str, status: &str, detail: &str) -> NestedChildRow {
    NestedChildRow {
        label: label.to_owned(),
        name: name.to_owned(),
        status_text: status.to_owned(),
        detail: detail.to_owned(),
    }
}
