//! `run_sqlload_project`: a SQLLoad project's load from Python, with the GIL released while it runs.

use std::collections::BTreeMap;

use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python, pyfunction};
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods};
use serde_json::{Value, json};
use sqlload::adapters::session::models::SessionConnection;
use sqlload::loader::models::PlanOptions;

use crate::bindings::constants::{ADAPTER, DATABASE, DESCRIPTION, OPENER};
use crate::runs::main::run_project::run_project;
use crate::runs::models::LoadedTable;
use crate::sessions::main::session_connection::session_connection;

/// Load `project` (SQLLoad TOML), its targets writing through `sessions`, each `{adapter, database, opener, description}`; each table's outcome as JSON.
#[pyfunction]
#[pyo3(signature = (*, project, sessions, select))]
pub(crate) fn run_sqlload_project(
    py: Python<'_>,
    project: &str,
    sessions: &Bound<'_, PyDict>,
    select: Vec<String>,
) -> PyResult<String> {
    let mut connections: BTreeMap<String, SessionConnection> = BTreeMap::new();
    for (name, session) in sessions.iter() {
        let session: &Bound<'_, PyDict> = session.downcast()?;
        let opener: Py<PyAny> = required(session, OPENER)?.unbind();
        let connection = session_connection(
            &required(session, ADAPTER)?.extract::<String>()?,
            required(session, DATABASE)?.extract()?,
            (opener, required(session, DESCRIPTION)?.extract()?),
        )
        .map_err(PyRuntimeError::new_err)?;
        connections.insert(name.extract()?, connection);
    }
    let options = PlanOptions {
        select,
        ..PlanOptions::default()
    };
    let project: String = project.to_owned();
    let tables: Vec<LoadedTable> = py
        .detach(move || run_project(&project, connections, options))
        .map_err(PyRuntimeError::new_err)?;
    let rows: Vec<Value> = tables
        .into_iter()
        .map(|table| {
            json!({
                "table": table.table,
                "rows": table.loaded.as_ref().ok(),
                "error": table.loaded.as_ref().err(),
                "changes": table.changes,
                "retryable": table.retryable,
            })
        })
        .collect();
    Ok(Value::Array(rows).to_string())
}

fn required<'py>(session: &Bound<'py, PyDict>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    session
        .get_item(key)?
        .ok_or_else(|| PyKeyError::new_err(format!("a load session needs `{key}`")))
}
