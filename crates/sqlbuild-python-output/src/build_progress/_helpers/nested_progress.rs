//! One live spinner row for all running items and grouped completed rows of `sqb test`.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::nested_progress::models::NestedChildRow;

use crate::build_progress::_helpers::result_text::text;
use sqlbuild_python_base::boundary::models::KeywordArguments;

pub(crate) const MIN_NAME_WIDTH: usize = 50;
pub(crate) const ITEM_OPTIONS: (&[&str], &[&str]) = (
    &["group_name", "item_name"],
    &[
        "status_text",
        "detail",
        "error_code",
        "error_help",
        "error_message",
        "child_rows",
        "canonical_resource_name",
        "canonical_resource_id",
    ],
);

/// Mutable reporter state; only held briefly and never across Python calls.
#[derive(Default)]
pub(crate) struct ReporterState {
    pub(crate) printed_group: Option<String>,
    pub(crate) running: Vec<(String, String)>,
    pub(crate) frame: usize,
    pub(crate) line_active: bool,
    pub(crate) cursor_hidden: bool,
    pub(crate) stop_event: Option<Py<PyAny>>,
    pub(crate) thread: Option<Py<PyAny>>,
    /// Whether item progress waits for each item's result, which may abandon the attempt.
    pub(crate) holding: bool,
}

pub(crate) fn optional_argument(
    arguments: &KeywordArguments<'_>,
    name: &str,
) -> PyResult<Option<String>> {
    if !arguments.passed(name)? {
        return Ok(None);
    }
    let value: Bound<'_, PyAny> = arguments.value(name)?;
    if value.is_none() {
        return Ok(None);
    }
    value.extract().map(Some)
}

pub(crate) fn child_rows(arguments: &KeywordArguments<'_>) -> PyResult<Vec<NestedChildRow>> {
    if !arguments.passed("child_rows")? {
        return Ok(Vec::new());
    }
    let mut rows: Vec<NestedChildRow> = Vec::new();
    for row in arguments.value("child_rows")?.try_iter()? {
        let row: Bound<'_, PyAny> = row?;
        rows.push(NestedChildRow {
            label: text(&row, "label")?,
            name: text(&row, "name")?,
            status_text: text(&row, "status_text")?,
            detail: text(&row, "detail")?,
        });
    }
    Ok(rows)
}
