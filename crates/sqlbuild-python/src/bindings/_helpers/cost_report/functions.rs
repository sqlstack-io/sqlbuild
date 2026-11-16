//! The cost run store and cost reports as `_native` functions.

use std::path::PathBuf;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cost_report::main::cost_history_json::cost_history_json;
use sqlbuild_core::cost_report::main::cost_output_payload::cost_output_payload as output_payload;
use sqlbuild_core::cost_report::main::cost_report_json::cost_report_json;
use sqlbuild_core::cost_report::main::format_cost_breakdown::format_cost_breakdown as breakdown;
use sqlbuild_core::cost_report::main::format_cost_history::format_cost_history as history;
use sqlbuild_core::cost_report::main::select_cost_run::select_cost_run;
use sqlbuild_core::cost_report::models::{BreakdownOptions, CostRun};

use crate::bindings::_helpers::cost_report::records::{
    cost_run, cost_runs, record_object, record_tuple,
};
use crate::bindings::_helpers::cost_report::store;
use crate::bindings::_helpers::scopes::scope_report_objects::json_to_python;
use sqlbuild_python_base::boundary::models::KeywordArguments;

const BREAKDOWN_OPTIONS: (&[&str], &[&str]) = (
    &["record", "use_color"],
    &["limit", "terminal_width", "show_run_id"],
);
const FALLBACK_TERMINAL_COLUMNS: usize = 100;

fn optional_record<'py>(
    py: Python<'py>,
    run: Option<CostRun>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    run.map(|run| record_object(py, &run)).transpose()
}

/// Atomically persist one run's cost artifact and return its path.
#[pyfunction]
#[pyo3(signature = (*, project_dir, record))]
pub(crate) fn write_cost_run(
    py: Python<'_>,
    project_dir: PathBuf,
    record: &Bound<'_, PyAny>,
) -> PyResult<PathBuf> {
    store::write(py, &project_dir, &cost_run(record)?)
}

/// Read the run stored under an exact run ID, or `None`.
#[pyfunction]
#[pyo3(signature = (*, project_dir, run_id))]
pub(crate) fn read_cost_run<'py>(
    py: Python<'py>,
    project_dir: PathBuf,
    run_id: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let run: Option<CostRun> = store::read(py, &project_dir, run_id)?;
    optional_record(py, run)
}

/// Every readable persisted run, newest first.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn list_cost_runs<'py>(
    py: Python<'py>,
    project_dir: PathBuf,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(record_tuple(py, &store::list(py, &project_dir)?)?.into_any())
}

/// The latest finished run, or the only run whose ID starts with the selector.
#[pyfunction]
#[pyo3(signature = (*, project_dir, selector))]
pub(crate) fn resolve_cost_run<'py>(
    py: Python<'py>,
    project_dir: PathBuf,
    selector: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let runs: Vec<CostRun> = store::list(py, &project_dir)?;
    optional_record(py, select_cost_run(runs, selector))
}

/// `sqb cost --json` text of one run.
#[pyfunction]
pub(crate) fn format_cost_json(record: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(cost_report_json(&cost_run(record)?))
}

/// `sqb cost history --json` text of the shown runs.
#[pyfunction]
#[pyo3(signature = (*, records, matching_count = None))]
pub(crate) fn format_cost_history_json(
    records: &Bound<'_, PyAny>,
    matching_count: Option<usize>,
) -> PyResult<String> {
    Ok(cost_history_json(&cost_runs(records)?, matching_count))
}

/// The `sqb cost --json` document of one run, as a dictionary.
#[pyfunction]
#[pyo3(signature = (*, record))]
pub(crate) fn cost_output_payload(
    py: Python<'_>,
    record: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    json_to_python(py, output_payload(&cost_run(record)?), false)
}

/// The aligned human breakdown of one run.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn format_cost_breakdown(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "format_cost_breakdown", options, BREAKDOWN_OPTIONS)?;
    let limit: Bound<'_, PyAny> = arguments.value("limit")?;
    let width: Bound<'_, PyAny> = arguments.value("terminal_width")?;
    let terminal_width: usize = if width.is_truthy()? {
        width.extract()?
    } else {
        py.import("shutil")?
            .call_method1("get_terminal_size", ((FALLBACK_TERMINAL_COLUMNS, 24),))?
            .getattr("columns")?
            .extract()?
    };
    let show_run_id: bool =
        !arguments.passed("show_run_id")? || arguments.value("show_run_id")?.is_truthy()?;
    Ok(breakdown(
        &cost_run(&arguments.value("record")?)?,
        BreakdownOptions {
            use_color: arguments.value("use_color")?.is_truthy()?,
            limit: if limit.is_none() {
                None
            } else {
                Some(limit.extract()?)
            },
            terminal_width,
            show_run_id,
        },
    ))
}

/// The aligned human history of the shown runs.
#[pyfunction]
#[pyo3(signature = (*, records, matching_count, use_color))]
pub(crate) fn format_cost_history(
    records: &Bound<'_, PyAny>,
    matching_count: usize,
    use_color: bool,
) -> PyResult<String> {
    Ok(history(&cost_runs(records)?, matching_count, use_color))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(write_cost_run, module)?)?;
    module.add_function(wrap_pyfunction!(read_cost_run, module)?)?;
    module.add_function(wrap_pyfunction!(list_cost_runs, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_cost_run, module)?)?;
    module.add_function(wrap_pyfunction!(format_cost_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_cost_history_json, module)?)?;
    module.add_function(wrap_pyfunction!(cost_output_payload, module)?)?;
    module.add_function(wrap_pyfunction!(format_cost_breakdown, module)?)?;
    module.add_function(wrap_pyfunction!(format_cost_history, module)?)
}
