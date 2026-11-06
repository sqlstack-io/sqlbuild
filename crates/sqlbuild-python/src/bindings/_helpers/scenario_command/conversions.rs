//! Scenario executor results read into plain rows and runtime artifact inputs.

use sqlbuild_analysis::compiler::main::scenario_expected_comparison_sql::scenario_expected_comparison_sql;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_core::scenario_command::main::build_local_runtime_artifacts::build_local_runtime_artifacts;
use sqlbuild_core::scenario_command::main::build_runtime_artifacts::build_runtime_artifacts;
use sqlbuild_core::scenario_command::main::write_runtime_artifacts::write_runtime_artifacts;
use sqlbuild_core::scenario_command::models::{
    AssertionRow, CaptureRelationRow, CaptureRow, CodedText, ExpectationSql, ExpectedRow,
    FixtureStatements, FunctionStatements, LoadedRelation, LocalRuntimeInputs, ModelArtifactEntry,
    NamedStatements, RetainedInputs, RetainedRelation, ScenarioResultRow, WarehouseRuntimeInputs,
};
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_host::models::{ExecutionStatus, ScenarioRunResult};
use sqlbuild_host::types::ResultEnum;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;

const SUCCESS_STATUS: &str = "success";
const SQL_EVENT_KIND: &str = "sql";
const PASS_STATUS: &str = "PASS";
const FAIL_STATUS: &str = "FAIL";
const RUN_DIR: &str = "run";
const SCENARIOS_DIR: &str = "scenarios";
const LOCAL_DIR: &str = "local";

fn items<'py>(value: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    value.try_iter()?.collect()
}

fn enum_value(value: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(value, "value")?.extract()
}

/// `result.status` is the given `ExecutionStatus` value.
pub(crate) fn matching_status(result: &Bound<'_, PyAny>, status: &str) -> PyResult<bool> {
    Ok(enum_value(&native_field(result, "status")?)? == status)
}

pub(crate) fn succeeded(result: &Bound<'_, PyAny>) -> PyResult<bool> {
    matching_status(result, SUCCESS_STATUS)
}

/// `path.as_posix()`.
fn posix(path: &Bound<'_, PyAny>) -> PyResult<String> {
    path.call_method0("as_posix")?.extract()
}

/// The result's coded error, when its message is set.
fn coded_error(result: &Bound<'_, PyAny>) -> PyResult<Option<CodedText>> {
    let message: Option<String> = native_field(result, "error_message")?.extract()?;
    let Some(message) = message else {
        return Ok(None);
    };
    Ok(Some(CodedText {
        code: native_field(result, "error_code")?.extract()?,
        message,
        help: native_field(result, "error_help")?.extract()?,
    }))
}

fn result_error(
    (code, message, help): (&Option<String>, &Option<String>, &Option<String>),
) -> Option<CodedText> {
    message.as_ref().map(|message| CodedText {
        code: code.clone(),
        message: message.clone(),
        help: help.clone(),
    })
}

/// One `ScenarioRunResult` as its result row shows it.
pub(crate) fn scenario_result_row(
    py: Python<'_>,
    result: &ScenarioRunResult<Bound<'_, PyAny>>,
) -> PyResult<ScenarioResultRow> {
    Ok(ScenarioResultRow {
        name: result.scenario_name.clone(),
        status: result
            .local_status
            .map_or_else(
                || {
                    if result.status == ExecutionStatus::Success {
                        PASS_STATUS
                    } else {
                        FAIL_STATUS
                    }
                },
                |status| status.value(),
            )
            .to_owned(),
        local: result.local_status.is_some(),
        duration_ms: result.duration_ms,
        error: result_error((
            &result.error_code,
            &result.error_message,
            &result.error_help,
        )),
        retained: result.retained,
        retained_relations: result.retained_relations.clone(),
        local_duckdb_path: result.local_duckdb_path.as_ref().map(|path| {
            path.to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/")
        }),
        expected: result
            .expected_results
            .iter()
            .map(|expected| ExpectedRow {
                model_name: expected.model_name.clone(),
                failed: expected.status == ExecutionStatus::Failed,
                actual_row_count: expected.actual_row_count,
                expected_row_count: expected.expected_row_count,
                mismatched_row_count: expected.mismatched_row_count,
                missing_row_count: expected.missing_row_count,
                error: result_error((
                    &expected.error_code,
                    &expected.error_message,
                    &expected.error_help,
                )),
            })
            .collect(),
        assertions: result
            .assertion_results
            .iter()
            .map(|assertion| {
                let first_sample = match assertion.sample_rows.first() {
                    Some(row) => Some(PyTuple::new(py, row)?.str()?.extract()?),
                    None => None,
                };
                Ok(AssertionRow {
                    name: assertion.name.clone(),
                    failed: assertion.status == ExecutionStatus::Failed,
                    failing_row_count: assertion.failing_row_count,
                    first_sample,
                    error: result_error((
                        &assertion.error_code,
                        &assertion.error_message,
                        &assertion.error_help,
                    )),
                })
            })
            .collect::<PyResult<_>>()?,
    })
}

/// `manifest_path` relative to the project when it is inside it, as a POSIX path.
fn display_path(
    manifest_path: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<String> {
    match manifest_path.call_method1("relative_to", (project_dir,)) {
        Ok(relative) => posix(&relative),
        Err(error)
            if error.is_instance_of::<pyo3::exceptions::PyValueError>(manifest_path.py()) =>
        {
            posix(manifest_path)
        }
        Err(error) => Err(error),
    }
}

/// One `ScenarioSnapshotCaptureRunResult` as capture and sync rows show it.
pub(crate) fn capture_row(
    result: &Bound<'_, PyAny>,
    (project_dir, scenario_plan): (&Bound<'_, PyAny>, Option<&Bound<'_, PyAny>>),
) -> PyResult<CaptureRow> {
    let retained: bool = native_field(result, "retained")?.is_truthy()?;
    let capture: Bound<'_, PyAny> = native_field(result, "capture_result")?;
    let (relations, snapshot_path) = if capture.is_none() {
        (None, None)
    } else {
        let relations: Vec<CaptureRelationRow> =
            items(&native_field(&capture, "relation_results")?)?
                .iter()
                .map(|relation| {
                    Ok(CaptureRelationRow {
                        kind: enum_value(&native_field(relation, "kind")?)?,
                        logical_name: native_field(relation, "logical_name")?.extract()?,
                        passed: succeeded(relation)?,
                        row_count: native_field(relation, "row_count")?.extract()?,
                        byte_count: native_field(relation, "byte_count")?.extract()?,
                    })
                })
                .collect::<PyResult<_>>()?;
        let manifest_path: Bound<'_, PyAny> = native_field(&capture, "manifest_path")?;
        let snapshot_path: Option<String> = if manifest_path.is_none() {
            None
        } else {
            Some(display_path(&manifest_path, project_dir)?)
        };
        (Some(relations), snapshot_path)
    };
    let retained_inputs: Option<RetainedInputs> = if retained && scenario_plan.is_some() {
        Some(RetainedInputs {
            fixtures: items(&native_field(result, "fixture_results")?)?
                .iter()
                .map(|fixture| {
                    Ok(RetainedRelation {
                        kind: enum_value(&native_field(fixture, "kind")?)?,
                        logical_name: native_field(fixture, "logical_name")?.extract()?,
                        physical_name: native_field(fixture, "target_relation")?.extract()?,
                    })
                })
                .collect::<PyResult<_>>()?,
            seeds: items(&native_field(result, "seed_results")?)?
                .iter()
                .map(|seed| native_field(seed, "seed_name")?.extract())
                .collect::<PyResult<_>>()?,
        })
    } else {
        None
    };
    Ok(CaptureRow {
        name: native_field(result, "scenario_name")?.extract()?,
        passed: succeeded(result)?,
        error: coded_error(result)?,
        retained,
        relations,
        snapshot_path,
        retained_inputs,
    })
}

/// The SQL contents of an executed result's lifecycle events.
fn sql_statements(result: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut statements: Vec<String> = Vec::new();
    for event in items(&native_field(result, "lifecycle_events")?)? {
        if enum_value(&native_field(&event, "kind")?)? == SQL_EVENT_KIND {
            statements.push(native_field(&event, "content")?.extract()?);
        }
    }
    Ok(statements)
}

fn cleanup_statements(cleanup: &Bound<'_, PyAny>) -> PyResult<Option<Vec<String>>> {
    if cleanup.is_none() {
        return Ok(None);
    }
    sql_statements(cleanup).map(Some)
}

/// Each expected comparison and assertion of a plan, rendered for `adapter`.
fn expectation_sql(
    scenario_plan: &Bound<'_, PyAny>,
    adapter: &Bound<'_, PyAny>,
) -> PyResult<ExpectationSql> {
    let py: Python<'_> = adapter.py();
    let mut expectations: ExpectationSql = ExpectationSql::default();
    for expected in items(&native_field(scenario_plan, "expected_expectations")?)? {
        let actual_relation: String = call_with(
            &host::entry(py, AdapterHost::ResolveRelationLocationQualifiedName)?,
            &[
                ("adapter", adapter.clone()),
                ("location", native_field(&expected, "actual_destination")?),
            ],
        )?
        .extract()?;
        let compared: Option<Vec<String>> =
            native_field(&expected, "compared_columns")?.extract()?;
        let sql: String = scenario_expected_comparison_sql(
            &format!("SELECT * FROM {actual_relation}"),
            &native_field(&expected, "expected_sql")?.extract::<String>()?,
            compared.as_deref(),
            &adapter
                .call_method0("render_set_difference_operator")?
                .extract::<String>()?,
        );
        expectations
            .expected
            .push((native_field(&expected, "model_name")?.extract()?, sql));
    }
    for assertion in items(&native_field(scenario_plan, "assertion_expectations")?)? {
        expectations.assertions.push((
            native_field(&assertion, "name")?.extract()?,
            native_field(&assertion, "sql")?.extract()?,
        ));
    }
    Ok(expectations)
}

fn scenario_run_dir(target_dir: &Path, scenario_plan: &Bound<'_, PyAny>) -> PyResult<PathBuf> {
    let name: String = native_field(scenario_plan, "name")?.extract()?;
    Ok(target_dir.join(RUN_DIR).join(SCENARIOS_DIR).join(name))
}

fn written(root: &Path, result: std::io::Result<()>) -> PyResult<()> {
    result.map_err(|error| PyOSError::new_err(format!("{error}: {}", root.display())))
}

/// Write executed warehouse scenario lifecycle SQL under `target/run/scenarios/<name>/`.
pub(crate) fn write_warehouse_runtime_target(
    target_dir: &Path,
    (adapter, scenario_plan, result): (&Bound<'_, PyAny>, &Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<()> {
    let named = |values: Bound<'_, PyAny>, field: &str| -> PyResult<Vec<NamedStatements>> {
        items(&values)?
            .iter()
            .map(|value| {
                Ok(NamedStatements {
                    name: native_field(value, field)?.extract()?,
                    statements: sql_statements(value)?,
                })
            })
            .collect()
    };
    let inputs: WarehouseRuntimeInputs = WarehouseRuntimeInputs {
        prepare_cleanup: cleanup_statements(&native_field(result, "prepare_cleanup_result")?)?,
        fixtures: items(&native_field(result, "fixture_results")?)?
            .iter()
            .map(|fixture| {
                Ok(FixtureStatements {
                    kind: enum_value(&native_field(fixture, "kind")?)?,
                    logical_name: native_field(fixture, "logical_name")?.extract()?,
                    statements: sql_statements(fixture)?,
                })
            })
            .collect::<PyResult<_>>()?,
        seeds: named(native_field(result, "seed_results")?, "seed_name")?,
        models: named(native_field(result, "model_results")?, "model_name")?,
        model_entries: items(&native_field(scenario_plan, "model_entries")?)?
            .iter()
            .map(|entry| {
                Ok(ModelArtifactEntry {
                    name: native_field(entry, "name")?.extract()?,
                    relative_path: native_field(entry, "relative_path")?.extract()?,
                })
            })
            .collect::<PyResult<_>>()?,
        expectations: expectation_sql(scenario_plan, adapter)?,
        final_cleanup: cleanup_statements(&native_field(result, "cleanup_result")?)?,
    };
    let root: PathBuf = scenario_run_dir(target_dir, scenario_plan)?;
    written(
        &root,
        write_runtime_artifacts(&root, &build_runtime_artifacts(&inputs)),
    )
}

fn function_statements(
    entry: &Bound<'_, PyAny>,
    adapter: &Bound<'_, PyAny>,
) -> PyResult<FunctionStatements> {
    let destination: Bound<'_, PyAny> = native_field(entry, "destination")?;
    let qualified: Bound<'_, PyAny> = native_field(&destination, "qualified_name")?;
    let destination_name: Bound<'_, PyAny> = if qualified.is_truthy()? {
        qualified
    } else {
        native_field(&destination, "name")?
    };
    let language: Bound<'_, PyAny> = native_field(entry, "language")?;
    let rendered: Bound<'_, PyAny> = call_with(
        &native_field(adapter, "render_create_function")?,
        &[
            ("destination", destination_name),
            ("arguments", native_field(entry, "arguments")?),
            ("returns", native_field(entry, "returns")?),
            ("body_sql", native_field(entry, "body_sql")?),
            ("return_columns", native_field(entry, "return_columns")?),
            ("language", language.clone()),
            ("runtime_version", native_field(entry, "runtime_version")?),
            ("entry_point", native_field(entry, "entry_point")?),
            ("packages", native_field(entry, "packages")?),
        ],
    )?;
    Ok(FunctionStatements {
        relative_path: native_field(entry, "relative_path")?.extract()?,
        language: enum_value(&language)?,
        statements: items(&rendered)?
            .iter()
            .map(|statement| statement.extract())
            .collect::<PyResult<_>>()?,
    })
}

/// Write local replay SQL under `target/run/scenarios/<name>/local/`.
pub(crate) fn write_local_runtime_target(
    target_dir: &Path,
    (adapter, scenario_plan, result): (&Bound<'_, PyAny>, &Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<()> {
    let local_plan: Bound<'_, PyAny> = match native_field(result, "local_execution_plan")? {
        plan if plan.is_truthy()? => plan,
        _ => scenario_plan.clone(),
    };
    let executed: HashSet<String> = items(&native_field(result, "function_results")?)?
        .iter()
        .map(|function| native_field(function, "function_name")?.extract())
        .collect::<PyResult<_>>()?;
    let mut functions: Vec<FunctionStatements> = Vec::new();
    for entry in items(&native_field(&local_plan, "function_entries")?)? {
        if executed.contains(&native_field(&entry, "name")?.extract::<String>()?) {
            functions.push(function_statements(&entry, adapter)?);
        }
    }
    let inputs: LocalRuntimeInputs = LocalRuntimeInputs {
        loaded_relations: items(&native_field(result, "local_snapshot_relations")?)?
            .iter()
            .map(|relation| {
                Ok(LoadedRelation {
                    kind: enum_value(&native_field(relation, "kind")?)?,
                    logical_name: native_field(relation, "logical_name")?.extract()?,
                    file_path: posix(&native_field(relation, "file_path")?)?,
                    table_name: native_field(relation, "table_name")?.extract()?,
                    row_count: native_field(relation, "row_count")?.extract()?,
                })
            })
            .collect::<PyResult<_>>()?,
        functions,
        models: items(&native_field(&local_plan, "model_entries")?)?
            .iter()
            .map(|entry| {
                Ok((
                    native_field(entry, "relative_path")?.extract()?,
                    native_field(entry, "resolved_sql")?.extract()?,
                ))
            })
            .collect::<PyResult<_>>()?,
        expectations: expectation_sql(&local_plan, adapter)?,
    };
    let root: PathBuf = scenario_run_dir(target_dir, scenario_plan)?.join(LOCAL_DIR);
    written(
        &root,
        write_runtime_artifacts(&root, &build_local_runtime_artifacts(&inputs)),
    )
}
