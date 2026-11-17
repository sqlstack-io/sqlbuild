//! The text plan grouped by reason with inline detail, optionally closed by its completion line.

use crate::plan_output::_helpers::plan_text::PLAN_OPTIONS;
use crate::plan_output::_helpers::plan_text::report;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_core::cli_text::main::format_plan_completion::format_plan_completion;
use sqlbuild_core::cli_text::main::format_plan_text::format_plan_text;
use sqlbuild_core::cli_text::models::PlanTextReport;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// The text plan grouped by reason with inline detail, optionally closed by its completion line.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn format_plan(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<String> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "format_plan", options, PLAN_OPTIONS)?;
    let report: PlanTextReport = report(&arguments)?;
    let mut text: String = format_plan_text(&report);
    if arguments.passed("changed_sections")? {
        let sections: String = arguments.value("changed_sections")?.extract()?;
        text = format!("{text}\n\n{sections}");
    }
    if !arguments.flag("include_completion")? {
        return Ok(text);
    }
    Ok(format!("{text}\n\n{}", format_plan_completion(&report)))
}
