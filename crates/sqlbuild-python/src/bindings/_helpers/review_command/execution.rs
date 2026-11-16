//! The clone, build and unit-test runs of a review, each read back from its JSON document.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBool;
use serde_json::Value;
use sqlbuild_analysis::review_command::constants::{
    CLONE_START, FULL_REFRESH_START, SEED_KIND, UNIT_TEST_LABEL,
};
use sqlbuild_analysis::review_command::models::{IncrementalStart, ReviewGraph, ValidationCounts};

use crate::bindings::_helpers::build_command::command::run_build_command;
use crate::bindings::_helpers::clone::clone_command::run_clone_command;
use crate::bindings::_helpers::dagster_invocation::access::string_tuple as texts;
use crate::bindings::_helpers::review_command::planning::ReviewSettings;
use crate::bindings::_helpers::review_command::support::{quietly, review_error};
use crate::bindings::_helpers::sql_test_command::test_command::run_test_command;
use sqlbuild_python_base::build_command::main::boolean::boolean;
use sqlbuild_python_base::build_command::main::text::text;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::contracts::main::resolve_target_config::resolve_target_config;

const FAILED_STATUS: &str = "failed";
const AUDIT_KIND: &str = "audit";
const SQL_TEST_KIND: &str = "sql_test";
const ERROR_SEVERITY: &str = "error";

/// What the review build did to each node, and its audit results.
#[derive(Default)]
pub(crate) struct BuildOutcome {
    /// Build status by node name.
    pub(crate) statuses: BTreeMap<String, String>,
    /// Nodes whose failure came from a blocking audit rather than from running them.
    pub(crate) audit_blocked: Vec<String>,
    pub(crate) audits: ValidationCounts,
    /// `(node, audit)` of every failed audit.
    pub(crate) failed_audits: Vec<(String, String)>,
}

/// The run's shared inputs to the sub-runs, borrowed for the length of the run.
pub(crate) struct RunInputs<'a, 'py> {
    pub(crate) request: &'a Bound<'py, PyAny>,
    pub(crate) project_dir: &'a Path,
    pub(crate) review_discovered: &'a Bound<'py, PyAny>,
    pub(crate) review_target: &'a str,
    pub(crate) work_dir: &'a Path,
    /// The progress stream, and whether stdout carries `--json`.
    pub(crate) output: (&'a Bound<'py, PyAny>, bool),
}

/// The JSON result document a sub-run wrote, or `None` when it wrote none.
fn read_document(path: &Path) -> PyResult<Option<Value>> {
    let text: String = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(pyo3::exceptions::PyOSError::new_err(error.to_string())),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))
}

fn entries<'a>(document: &'a Value, key: &str) -> &'a [Value] {
    document
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn field<'a>(item: &'a Value, key: &str) -> &'a str {
    item.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn path_object<'py>(py: Python<'py>, path: &Path) -> PyResult<Bound<'py, PyAny>> {
    path.into_pyobject(py)
}

/// How each changed stateful model starts: cloned from `--against` when that is free, else full.
pub(crate) fn incremental_starts(
    inputs: &RunInputs<'_, '_>,
    (settings, adapter): (&ReviewSettings, &Bound<'_, PyAny>),
    stateful: &[String],
) -> PyResult<Vec<IncrementalStart>> {
    let py: Python<'_> = adapter.py();
    if stateful.is_empty() {
        return Ok(Vec::new());
    }
    let full = |reason: &str| -> Vec<IncrementalStart> {
        stateful
            .iter()
            .map(|node| IncrementalStart {
                node: node.clone(),
                method: FULL_REFRESH_START.to_owned(),
                reason: reason.to_owned(),
            })
            .collect()
    };
    if native_field(inputs.request, "no_clone")?.is_truthy()? {
        return Ok(full("cloning opted out with --no-clone"));
    }
    if !adapter
        .call_method0("supports_zero_copy_clone")?
        .is_truthy()?
    {
        return Ok(full("the adapter has no zero-copy clone"));
    }
    let against: Bound<'_, PyAny> = resolve_target_config(
        py,
        &native_field(inputs.review_discovered, "project_config")?,
        &native_field(inputs.review_discovered, "local_config")?,
        &text(py, &settings.against),
    )?;
    if !native_field(&native_field(&against, "clone")?, "allow_as_clone_origin")?.is_truthy()? {
        return Ok(full(&format!(
            "target '{}' does not allow cloning (allow_as_clone_origin)",
            settings.against
        )));
    }
    let output: PathBuf = inputs.work_dir.join("clone.json");
    let request: Bound<'_, PyAny> = new_record_with(
        py,
        RecordId::CloneCommandRequest,
        &[
            ("project_dir", path_object(py, inputs.project_dir)?),
            ("no_color", native_field(inputs.request, "no_color")?),
            ("no_sql_validation", boolean(py, true)),
            ("origin_target_name", text(py, &settings.against)),
            ("destination_target_name", text(py, inputs.review_target)),
            ("hard_copy", boolean(py, false)),
            ("select", texts(py, stateful)?),
            ("cli_vars", native_field(inputs.request, "cli_vars")?),
            ("json_output_path", path_object(py, &output)?),
            ("discovered_inputs", inputs.review_discovered.clone()),
        ],
    )?;
    let code: i32 = quietly(inputs.output, || run_clone_command(py, &request))?;
    if code != 0 {
        return Err(review_error(
            py,
            "C772",
            &format!(
                "cloning changed incremental models from '{}' failed",
                settings.against
            ),
            Some("rerun with --no-clone to build them with a full refresh"),
        ));
    }
    Ok(stateful
        .iter()
        .map(|node| IncrementalStart {
            node: node.clone(),
            method: CLONE_START.to_owned(),
            reason: format!("zero-copy clone of {}", settings.against),
        })
        .collect())
}

/// The selector of one planned node.
fn selector(graph: &ReviewGraph, name: &str) -> String {
    match graph.nodes.get(name) {
        Some(node) if node.kind == SEED_KIND => format!("seed:{name}"),
        _ => name.to_owned(),
    }
}

/// Build the validated nodes into the review schema with deferral to `--against`.
pub(crate) fn build_validated(
    inputs: &RunInputs<'_, '_>,
    settings: &ReviewSettings,
    (graph, validated): (&ReviewGraph, &[String]),
) -> PyResult<BuildOutcome> {
    let py: Python<'_> = inputs.request.py();
    let output: PathBuf = inputs.work_dir.join("build.json");
    let selectors: Vec<String> = validated.iter().map(|name| selector(graph, name)).collect();
    let request: Bound<'_, PyAny> = new_record_with(
        py,
        RecordId::BuildCommandRequest,
        &[
            ("project_dir", path_object(py, inputs.project_dir)?),
            (
                "no_sql_validation",
                native_field(inputs.request, "no_sql_validation")?,
            ),
            ("defer_to", text(py, &settings.against)),
            ("selected_target", text(py, inputs.review_target)),
            ("no_color", native_field(inputs.request, "no_color")?),
            ("load_sources", boolean(py, false)),
            ("include_python", boolean(py, false)),
            ("allow_missing_migration_origin", boolean(py, true)),
            ("concurrency", native_field(inputs.request, "concurrency")?),
            ("select", texts(py, &selectors)?),
            ("verbose", native_field(inputs.request, "verbose")?),
            ("debug", native_field(inputs.request, "debug")?),
            ("cli_vars", native_field(inputs.request, "cli_vars")?),
            ("run_tests", boolean(py, false)),
            ("run_audits", boolean(py, true)),
            ("json_output_path", path_object(py, &output)?),
            ("discovered_inputs", inputs.review_discovered.clone()),
        ],
    )?;
    let ran: PyResult<i32> = quietly(inputs.output, || run_build_command(py, &request));
    let document: Option<Value> = read_document(&output)?;
    let Some(document) = document else {
        ran?;
        return Err(review_error(
            py,
            "C773",
            "the review build wrote no result document",
            None,
        ));
    };
    if let Err(error) = ran {
        let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
        if value.getattr_opt("code")?.is_none() {
            return Err(error);
        }
    }
    Ok(build_outcome(&document))
}

fn build_outcome(document: &Value) -> BuildOutcome {
    let mut outcome: BuildOutcome = BuildOutcome::default();
    for asset in entries(document, "assets") {
        let name: &str = field(asset, "name");
        if name.is_empty() {
            continue;
        }
        let status: &str = field(asset, "status");
        let phase: &str = asset
            .get("failed_phase")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if status == FAILED_STATUS && phase.contains(AUDIT_KIND) {
            outcome.audit_blocked.push(name.to_owned());
        }
        outcome.statuses.insert(name.to_owned(), status.to_owned());
    }
    for check in entries(document, "checks") {
        if field(check, "kind") != AUDIT_KIND {
            continue;
        }
        outcome.audits.selected += 1;
        outcome.audits.total += 1;
        let passed: bool = check
            .get("passed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if passed {
            outcome.audits.passed += 1;
            continue;
        }
        outcome.audits.failed += 1;
        let node: String = field(check, "asset_name").to_owned();
        let audit: String = field(check, "name").to_owned();
        let severity: &str = field(check, "severity");
        let label: String = if severity.is_empty() || severity == ERROR_SEVERITY {
            format!("audit {audit} on {node}")
        } else {
            format!("audit {audit} on {node} ({severity})")
        };
        outcome.audits.failures.push(label);
        outcome.failed_audits.push((node, audit));
    }
    outcome
}

/// Run the unit tests that execute something changed since `--since`, as `sqb test --since` does.
pub(crate) fn run_changed_tests(
    inputs: &RunInputs<'_, '_>,
    settings: &ReviewSettings,
) -> PyResult<ValidationCounts> {
    let py: Python<'_> = inputs.request.py();
    let output: PathBuf = inputs.work_dir.join("tests.json");
    let request: Bound<'_, PyAny> = new_record_with(
        py,
        RecordId::TestCommandRequest,
        &[
            ("project_dir", path_object(py, inputs.project_dir)?),
            (
                "no_sql_validation",
                native_field(inputs.request, "no_sql_validation")?,
            ),
            ("no_color", native_field(inputs.request, "no_color")?),
            ("selected_target", text(py, &settings.target)),
            ("cli_vars", native_field(inputs.request, "cli_vars")?),
            ("json_output_path", path_object(py, &output)?),
            ("concurrency", native_field(inputs.request, "concurrency")?),
            ("since", text(py, &settings.since)),
            ("sql_only", PyBool::new(py, true).to_owned().into_any()),
        ],
    )?;
    let ran: PyResult<i32> = quietly(inputs.output, || run_test_command(py, &request));
    let Some(document) = read_document(&output)? else {
        ran?;
        return Ok(ValidationCounts::default());
    };
    ran.or_else(|error| {
        let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
        if value.getattr_opt("code")?.is_none() {
            Err(error)
        } else {
            Ok(1)
        }
    })?;
    let mut counts: ValidationCounts = ValidationCounts::default();
    for check in entries(&document, "checks") {
        if field(check, "kind") != SQL_TEST_KIND {
            continue;
        }
        counts.selected += 1;
        if check
            .get("passed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            counts.passed += 1;
        } else {
            counts.failed += 1;
            counts
                .failures
                .push(format!("{UNIT_TEST_LABEL}{}", field(check, "name")));
        }
    }
    counts.total = document
        .get("since")
        .and_then(|since| since.get("candidate_count"))
        .and_then(Value::as_u64)
        .map_or(counts.selected, |total| {
            usize::try_from(total).unwrap_or(usize::MAX)
        });
    Ok(counts)
}
