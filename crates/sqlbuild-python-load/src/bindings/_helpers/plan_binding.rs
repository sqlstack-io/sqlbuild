//! `plan_sqlload_project`: SQLLoad's plan of a project from Python, with the GIL released while it plans.

use pyo3::prelude::{Bound, PyResult, pyfunction};
use pyo3::types::PyDict;
use serde_json::json;

use crate::bindings::_helpers::run_binding::detached_load;
use crate::runs::main::plan_project::plan_project;
use crate::runs::models::PlannedProject;

/// Plan `project` (SQLLoad TOML), its targets writing through `sessions`; `{lines, ready, tables, missing}` as JSON.
#[pyfunction]
#[pyo3(signature = (*, project, sessions, tables, reload = false))]
pub(crate) fn plan_sqlload_project(
    project: &str,
    sessions: &Bound<'_, PyDict>,
    tables: (Vec<String>, Vec<String>),
    reload: bool,
) -> PyResult<String> {
    let planned: PlannedProject = detached_load(
        (project, sessions),
        (tables, reload),
        |project, (connections, options, require)| {
            plan_project(&project, connections, (options, require))
        },
    )?;
    Ok(json!({
        "lines": planned.lines,
        "ready": planned.ready,
        "tables": planned.tables,
        "missing": planned.missing,
    })
    .to_string())
}
