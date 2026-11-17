//! Failure descriptions and metadata of failed SQLBuild CLI invocations.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyString};
use sqlbuild_core::dagster_invocation::main::asset_failure_text::asset_failure_text;
use sqlbuild_core::dagster_invocation::main::diagnostic_tail::diagnostic_tail;
use sqlbuild_core::dagster_invocation::main::failure_summary::failure_summary;
use sqlbuild_core::dagster_invocation::models::{AssetFailure, FailedCheck};
use sqlbuild_host::entries::models::ThirdPartyHost;

use crate::bindings::_helpers::dagster_invocation::access::{
    constant_contains, get, items, text_of,
};
use crate::bindings::_helpers::dagster_invocation::results::{SKIPPED_CHECK_STATUS, warn_severity};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Failed checks listed in failure metadata; `failed_check_count` counts them all.
const MAX_FAILED_CHECK_METADATA: usize = 100;
const LIVE_FAILURE_MESSAGE: &str =
    "SQLBuild resource attempt failed; see final execution diagnostics";
const UNKNOWN: &str = "unknown";

fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.extract()?))
}

fn sql_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if !value.is_instance_of::<PyString>() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

fn asset_failure(asset: &Bound<'_, PyAny>) -> PyResult<AssetFailure> {
    Ok(AssetFailure {
        kind: text_of(asset, "kind")?,
        name: text_of(asset, "name")?,
        status: text_of(asset, "status")?,
        failed_phase: optional_text(&get(asset, "failed_phase")?)?,
        error_message: optional_text(&get(asset, "error_message")?)?,
        error_code: optional_text(&get(asset, "error_code")?)?,
        staging_relation: optional_text(&get(asset, "staging_relation")?)?,
        failed_sql: sql_text(&get(asset, "failed_sql")?)?,
        last_recorded_sql: sql_text(&get(asset, "last_recorded_sql")?)?,
    })
}

/// A check of the execution document that failed with error severity, or `None`.
fn failed_check(check: &Bound<'_, PyAny>) -> PyResult<Option<FailedCheck>> {
    if get(check, "passed")?.is_truthy()?
        || get(check, "status")?.eq(SKIPPED_CHECK_STATUS)?
        || warn_severity(check)?
    {
        return Ok(None);
    }
    let row_count: Bound<'_, PyAny> = get(check, "row_count")?;
    Ok(Some(FailedCheck {
        kind: text_of(check, "kind")?,
        name: text_of(check, "name")?,
        asset_name: optional_text(&get(check, "asset_name")?)?,
        column_name: optional_text(&get(check, "attached_column_name")?)?,
        status: text_of(check, "status")?,
        row_count: row_count.extract()?,
    }))
}

fn failed_check_metadata<'py>(
    py: Python<'py>,
    checks: &[FailedCheck],
) -> PyResult<Bound<'py, PyList>> {
    let listed: Bound<'py, PyList> = PyList::empty(py);
    for check in checks.iter().take(MAX_FAILED_CHECK_METADATA) {
        let item: Bound<'py, PyDict> = PyDict::new(py);
        item.set_item("kind", &check.kind)?;
        item.set_item("name", &check.name)?;
        item.set_item("asset", check.asset_name.as_deref())?;
        item.set_item("column", check.column_name.as_deref())?;
        item.set_item("status", &check.status)?;
        item.set_item("failing_rows", check.row_count)?;
        listed.append(item)?;
    }
    Ok(listed)
}

/// The Dagster failure description and metadata of a failed invocation.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn dagster_failure<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<(String, Bound<'py, PyDict>)> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "dagster_failure",
        options,
        (
            &[
                "command",
                "project_dir",
                "selection",
                "selector_file",
                "returncode",
                "execution_payload",
                "stdout",
                "stderr",
            ],
            &[],
        ),
    )?;
    let command: Vec<String> = arguments.value("command")?.extract()?;
    let project_dir: Bound<'py, PyAny> = arguments.value("project_dir")?;
    let selection: Vec<String> = arguments.value("selection")?.extract()?;
    let selector_file: String = arguments.value("selector_file")?.extract()?;
    let returncode: i64 = arguments.value("returncode")?.extract()?;
    let payload_value: Bound<'py, PyAny> = arguments.value("execution_payload")?;
    let execution_payload: Option<Bound<'py, PyAny>> =
        (!payload_value.is_none()).then_some(payload_value);
    let stdout: String = arguments.value("stdout")?.extract()?;
    let stderr: String = arguments.value("stderr")?.extract()?;
    let command_text: String = command.join(" ");
    let metadata: Bound<'py, PyDict> = PyDict::new(py);
    metadata.set_item("command", &command_text)?;
    metadata.set_item("project_dir", project_dir.str()?)?;
    metadata.set_item("selection_count", selection.len())?;
    metadata.set_item("selector_file", selector_file)?;
    let mut description: String =
        format!("SQLBuild CLI command failed with exit code {returncode}: {command_text}");
    match execution_payload {
        Some(payload) => {
            let mut incomplete: Vec<String> = Vec::new();
            let mut failures: Vec<AssetFailure> = Vec::new();
            let mut details: Vec<String> = Vec::new();
            let limits: Bound<'py, PyDict> = PyDict::new(py);
            for asset in items(&payload, "assets")? {
                let status: String = text_of(&asset, "status")?;
                if constant_contains(py, (ThirdPartyHost::CompletedExecutionStatuses, &status))? {
                    continue;
                }
                let failure: AssetFailure = asset_failure(&asset)?;
                incomplete.push(format!("{}:{} ({status})", failure.kind, failure.name));
                details.push(asset_failure_text(&failure));
                failures.push(failure);
                let microbatch: Bound<'py, PyAny> = get(&asset, "microbatch")?;
                if microbatch.is_instance_of::<PyDict>() && !get(&microbatch, "limit")?.is_none() {
                    limits.set_item(text_of(&asset, "name")?, microbatch)?;
                }
            }
            let mut checks: Vec<FailedCheck> = Vec::new();
            for check in items(&payload, "checks")? {
                checks.extend(failed_check(&check)?);
            }
            metadata.set_item("execution_status", text_of(&payload, "status")?)?;
            metadata.set_item("incomplete_assets", incomplete.join(", "))?;
            metadata.set_item("failed_check_count", checks.len())?;
            metadata.set_item("failed_checks", failed_check_metadata(py, &checks)?)?;
            let summary: String = failure_summary(&checks, &failures);
            if !summary.is_empty() {
                description = format!("{summary}\n\n{description}");
            }
            if !limits.is_empty() {
                metadata.set_item("microbatch_limits", limits)?;
            }
            if !details.is_empty() {
                description.push_str("\n\nFailures:\n\n");
                description.push_str(&details.join("\n\n"));
            }
        }
        None => {
            let stdout_tail: String = diagnostic_tail(&stdout);
            let stderr_tail: String = diagnostic_tail(&stderr);
            if !stdout_tail.is_empty() {
                metadata.set_item("stdout_tail", &stdout_tail)?;
            }
            if !stderr_tail.is_empty() {
                metadata.set_item("stderr_tail", &stderr_tail)?;
            }
            let diagnostic: &str = match stderr_tail.trim() {
                "" => stdout_tail.trim(),
                trimmed => trimmed,
            };
            if !diagnostic.is_empty() {
                description.push_str(&format!("\n\n{diagnostic}"));
            }
        }
    }
    Ok((description, metadata))
}

/// The log arguments and structured fields of one failed live resource attempt.
#[pyfunction]
#[pyo3(signature = (*, asset, envelope, command))]
pub(crate) fn dagster_live_failure<'py>(
    asset: Bound<'py, PyAny>,
    envelope: Bound<'py, PyAny>,
    command: Vec<String>,
) -> PyResult<(Vec<String>, Bound<'py, PyDict>)> {
    let py: Python<'py> = asset.py();
    let kind: String = native_field(&asset, "kind")?.extract()?;
    let name: String = native_field(&asset, "name")?.extract()?;
    let label: String = format!("{kind}:{name}");
    let or_unknown = |value: Bound<'py, PyAny>| -> PyResult<String> {
        if value.is_truthy()? {
            value.str()?.extract()
        } else {
            Ok(UNKNOWN.to_owned())
        }
    };
    let phase: String = or_unknown(native_field(&asset, "failed_phase")?)?;
    let code: String = or_unknown(native_field(&envelope, "error_code")?)?;
    let metadata: Bound<'py, PyDict> = PyDict::new(py);
    metadata.set_item("sqlbuild_asset", &label)?;
    metadata.set_item("sqlbuild_asset_kind", &kind)?;
    metadata.set_item("sqlbuild_asset_name", &name)?;
    metadata.set_item("sqlbuild_phase", &phase)?;
    metadata.set_item("sqlbuild_error_code", &code)?;
    metadata.set_item("sqlbuild_error_message", LIVE_FAILURE_MESSAGE)?;
    metadata.set_item("sqlbuild_command", command.join(" "))?;
    for (key, value) in [
        ("sqlbuild_target", native_field(&asset, "target")?),
        (
            "sqlbuild_staging_relation",
            native_field(&asset, "staging_relation")?,
        ),
        (
            "sqlbuild_duration_ms",
            native_field(&envelope, "duration_ms")?,
        ),
    ] {
        if !value.is_none() {
            metadata.set_item(key, value)?;
        }
    }
    Ok((
        vec![label, phase, code, LIVE_FAILURE_MESSAGE.to_owned()],
        metadata,
    ))
}
