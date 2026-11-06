//! The permanent Python request/refresh edge of native cost reporting.

use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cost_command::constants::{
    DEFAULT_DETAIL_SORT, DEFAULT_HISTORY_SORT, DESCENDING_ORDER, HISTORY_SELECTOR, ISO_DATE_LENGTH,
};
use sqlbuild_core::cost_command::main::render_cost_report::render_cost_report;
use sqlbuild_core::cost_command::main::validate_cost_request::validate_cost_request;
use sqlbuild_core::cost_command::models::{CostReport, CostReportOptions};
use sqlbuild_core::cost_report::main::cost_timestamp::cost_timestamp;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::cost_command::bounds::{parse_bound, parse_since};
use crate::bindings::_helpers::cost_command::refresh::refresh_pending_cost_run;
use crate::bindings::_helpers::cost_command::refusal::cost_error;
use crate::bindings::_helpers::cost_report::store;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::records::fields::field;

/// Preserve Python's datetime parsing at the public request edge, then leave Python records behind.
fn instant(value: &Bound<'_, PyAny>) -> PyResult<i128> {
    let text: String = value.call_method0("isoformat")?.extract()?;
    cost_timestamp(&text)
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Invalid UTC cost bound"))
}

fn options(request: &Bound<'_, PyAny>) -> PyResult<CostReportOptions> {
    let py: Python<'_> = request.py();
    let selector: String = field(request, "selector")?.extract()?;
    let history: bool = selector == HISTORY_SELECTOR;
    let sort: Option<String> = field(request, "sort")?.extract()?;
    let sort: String = sort.filter(|sort| !sort.is_empty()).unwrap_or_else(|| {
        if history {
            DEFAULT_HISTORY_SORT
        } else {
            DEFAULT_DETAIL_SORT
        }
        .to_owned()
    });
    let limit: Option<i64> = field(request, "limit")?.extract()?;
    let no_limit: bool = field(request, "no_limit")?.is_truthy()?;
    let since: Option<String> = field(request, "since")?.extract()?;
    let until: Option<String> = field(request, "until")?.extract()?;
    validate_cost_request(
        &selector,
        (limit, no_limit),
        &sort,
        since.is_some() || until.is_some(),
    )
    .map_err(|error| cost_error(py, &error))?;
    let order: Option<String> = field(request, "order")?.extract()?;
    Ok(CostReportOptions {
        selector,
        limit,
        no_limit,
        sort,
        descending: order
            .filter(|order| !order.is_empty())
            .is_none_or(|order| order == DESCENDING_ORDER),
        since: since
            .as_deref()
            .map(|value| instant(&parse_since(py, value)?))
            .transpose()?,
        until: until
            .as_deref()
            .map(|value| instant(&parse_bound(py, value, "--until", true)?))
            .transpose()?,
        until_exclusive: until
            .as_deref()
            .is_some_and(|value| value.chars().count() == ISO_DATE_LENGTH),
        use_color: !field(request, "no_color")?.is_truthy()? && stdout_supports_color(py)?,
        terminal_width: py
            .import("shutil")?
            .call_method1("get_terminal_size", ((100, 24),))?
            .getattr("columns")?
            .extract()?,
    })
}

fn emit(request: &Bound<'_, PyAny>, report: CostReport) -> PyResult<()> {
    let py: Python<'_> = request.py();
    let path: Bound<'_, PyAny> = field(request, "json_output_path")?;
    if !path.is_none() {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("parents", true)?;
        options.set_item("exist_ok", true)?;
        field(&path, "parent")?.call_method("mkdir", (), Some(&options))?;
        let encoding: Bound<'_, PyDict> = PyDict::new(py);
        encoding.set_item("encoding", "utf-8")?;
        path.call_method(
            "write_text",
            (format!("{}\n", report.json),),
            Some(&encoding),
        )?;
    }
    let rendered: String = if field(request, "json_output")?.is_truthy()? {
        report.json
    } else {
        report.text
    };
    py.import("sys")?
        .getattr("stdout")?
        .call_method1("write", (format!("{rendered}\n"),))?;
    Ok(())
}

/// Refresh through the adapter edge, then render entirely from native persisted records.
#[pyfunction]
pub(crate) fn run_cost_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    refresh_pending_cost_run(py, request);
    let options: CostReportOptions = options(request)?;
    let requested: Bound<'_, PyAny> = field(request, "project_dir")?;
    let project_dir: PathBuf = if requested.is_none() {
        imported(py, "pathlib", "Path")?
            .call_method0("cwd")?
            .extract()?
    } else {
        requested.extract()?
    };
    let report: CostReport = render_cost_report(store::list(py, &project_dir)?, &options)
        .map_err(|error| cost_error(py, &error))?;
    emit(request, report)?;
    Ok(0)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_cost_command, module)?)
}
