//! Aggregate live state and plan-ordered result rows of standalone concurrent audits.

use std::collections::HashMap;

use pyo3::prelude::{Bound, Py, PyAny, PyResult};
use sqlbuild_core::nested_progress::main::audit_resource_id::audit_resource_id;

use crate::build_progress::_helpers::result_text::optional;
use crate::build_progress::_helpers::result_text::text;

/// The identity an audit plan entry or result reports to the progress projector.
pub(crate) fn resource_id(value: &Bound<'_, PyAny>, name_field: &str) -> PyResult<String> {
    let kind: Option<String> = match optional(value, "attached_target_kind")? {
        Some(kind) => Some(kind),
        None => Some(text(value, "attachment_kind")?),
    };
    Ok(audit_resource_id(
        &text(value, name_field)?,
        kind.as_deref().unwrap_or(""),
        (
            optional(value, "attached_target_name")?.as_deref(),
            optional(value, "attached_column_name")?.as_deref(),
        ),
    ))
}

/// Mutable reporter state; only held briefly and never across Python calls.
#[derive(Debug, Default)]
pub(crate) struct ReporterState {
    pub(crate) completed_by_index: HashMap<usize, Option<Py<PyAny>>>,
    pub(crate) next_index: usize,
    pub(crate) running: usize,
    pub(crate) completed: usize,
    pub(crate) current_group: Option<String>,
    pub(crate) closed: bool,
    pub(crate) cursor_hidden: bool,
    pub(crate) on_result: Option<Py<PyAny>>,
}
