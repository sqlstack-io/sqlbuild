//! The finished result of one rules run, the bulk cache locations and freed-memory release.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple, PyTupleMethods};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_rules::rule_cache::main::rules_bulk_cache_path::rules_bulk_cache_path as bulk_cache_path;
use sqlbuild_rules::rule_results::main::group_unevaluated::group_unevaluated;
use sqlbuild_rules::rule_results::models::RuleFinding;

use crate::bindings::_helpers::rules::fact_replay::project_models;
use crate::bindings::_helpers::rules::results::rule_finding;
use crate::bindings::_helpers::rules::rows::finalize_rule_findings_rows;
use crate::bindings::_helpers::rules::rule_plan::whole_project_codes;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as record_field;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const PATHLIB_MODULE: &str = "pathlib";
const CTYPES_MODULE: &str = "ctypes";

fn integer(object: &Bound<'_, PyAny>, field: &str) -> PyResult<i64> {
    record_field(object, field)?.extract()
}

fn python_finding<'py>(py: Python<'py>, finding: RuleFinding) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        py,
        RecordId::Finding,
        &[
            ("code", finding.code.into_bound_py_any(py)?),
            (
                "path",
                imported(py, PATHLIB_MODULE, "Path")?.call1((finding.path,))?,
            ),
            ("line", finding.line.into_bound_py_any(py)?),
            ("column", finding.column.into_bound_py_any(py)?),
            ("message", finding.message.into_bound_py_any(py)?),
            ("remediation", finding.remediation.into_bound_py_any(py)?),
            ("unevaluated", finding.unevaluated.into_bound_py_any(py)?),
            (
                "affected_rules",
                PyTuple::new(py, finding.affected_rules)?.into_any(),
            ),
            ("fixable", finding.fixable.into_bound_py_any(py)?),
        ],
    )
}

/// Finalize the combined SQL, built-in and custom findings of one planned rules run.
#[pyfunction]
#[pyo3(signature = (*, plan, sql_result, result))]
pub(crate) fn finish_rule_run<'py>(
    plan: &Bound<'py, PyAny>,
    sql_result: &Bound<'py, PyAny>,
    result: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan.py();
    let whole_project: Vec<String> = whole_project_codes(plan)?;
    let mut evaluated_codes: Vec<String> = Vec::new();
    for rule in record_field(plan, "selected")?.try_iter()? {
        let code: String = record_field(&rule?, "code")?.extract()?;
        if !whole_project.contains(&code) {
            evaluated_codes.push(code);
        }
    }
    let mut combined: Vec<Bound<'py, PyAny>> = Vec::new();
    for findings in [
        record_field(sql_result, "findings")?,
        record_field(result, "findings")?,
    ] {
        combined.extend(findings.try_iter()?.collect::<PyResult<Vec<_>>>()?);
    }
    let finalized: Vec<RuleFinding> = finalize_rule_findings_rows(
        &record_field(plan, "config")?,
        &record_field(plan, "project_dir")?,
        evaluated_codes,
        combined,
    )?
    .iter()
    .map(|finding| rule_finding(&finding))
    .collect::<PyResult<_>>()?;
    let grouped: Vec<RuleFinding> = compiler_guard(|| Ok(group_unevaluated(finalized)))?;
    let unevaluated: BTreeSet<String> = grouped
        .iter()
        .filter(|finding| finding.unevaluated)
        .map(|finding| finding.path.clone())
        .collect();
    let mut unevaluated_models: i64 = 0;
    if !unevaluated.is_empty() {
        for model in project_models(&record_field(plan, "project")?)? {
            unevaluated_models += i64::from(unevaluated.contains(&model.posix));
        }
    }
    let findings: Vec<Bound<'py, PyAny>> = grouped
        .into_iter()
        .map(|finding| python_finding(py, finding))
        .collect::<PyResult<_>>()?;
    let total = |field: &str| -> PyResult<Bound<'py, PyAny>> {
        (integer(result, field)? + integer(sql_result, field)?).into_bound_py_any(py)
    };
    new_record_with(
        py,
        RecordId::RulesRunResult,
        &[
            ("findings", PyTuple::new(py, findings)?.into_any()),
            (
                "evaluated_models",
                (integer(result, "evaluated_models")? - unevaluated_models)
                    .max(0)
                    .into_bound_py_any(py)?,
            ),
            (
                "unevaluated_resources",
                unevaluated.len().into_bound_py_any(py)?,
            ),
            (
                "built_in_ms",
                (integer(sql_result, "sql_ms")? + integer(result, "built_in_ms")?)
                    .into_bound_py_any(py)?,
            ),
            ("custom_ms", record_field(result, "custom_ms")?),
            ("custom_cpu_ms", record_field(result, "custom_cpu_ms")?),
            (
                "custom_rule_timings",
                record_field(result, "custom_rule_timings")?,
            ),
            ("cache_hits", total("cache_hits")?),
            ("cache_misses", total("cache_misses")?),
            (
                "skipped_type_proof_rules",
                record_field(plan, "skipped_type_proof_rules")?,
            ),
            ("skipped_models", record_field(result, "skipped_models")?),
        ],
    )
}

/// Run the selected built-in, SQL and custom rules; custom rules run on their worker threads.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_rules<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    crate::bindings::_helpers::rules::rule_dispatch::evaluate_rules(py, options)
}

/// One bulk Rules cache file owned by the native compiler engine.
#[pyfunction]
#[pyo3(signature = (*, project_dir, file_name))]
pub(crate) fn rules_bulk_cache_path(project_dir: PathBuf, file_name: &str) -> PathBuf {
    bulk_cache_path(&project_dir, file_name)
}

/// One bulk Rules cache file, inside `directory` of the bulk cache when given.
pub(crate) fn bulk_cache_file(
    project_dir: &Path,
    directory: Option<&str>,
    file_name: &str,
) -> PathBuf {
    match directory {
        Some(directory) => bulk_cache_path(project_dir, directory).join(file_name),
        None => bulk_cache_path(project_dir, file_name),
    }
}

/// Hand freed glibc heap pages back to the OS; a no-op elsewhere or without `malloc_trim`.
pub(crate) fn freed_memory_released(py: Python<'_>) -> PyResult<()> {
    if !cfg!(target_os = "linux") {
        return Ok(());
    }
    let trim = || -> PyResult<()> {
        let process: Bound<'_, PyAny> = imported(py, CTYPES_MODULE, "CDLL")?.call1((py.None(),))?;
        let _ = native_field(&process, "malloc_trim")?.call1((0,))?;
        Ok(())
    };
    match trim() {
        Err(error) if !error.is_instance_of::<PyException>(py) => Err(error),
        _ => Ok(()),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(finish_rule_run, module)?)?;
    module.add_function(wrap_pyfunction!(run_rules, module)?)?;
    module.add_function(wrap_pyfunction!(rules_bulk_cache_path, module)?)?;
    Ok(())
}
