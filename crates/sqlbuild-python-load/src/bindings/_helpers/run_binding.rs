//! `run_sqlload_project`: a SQLLoad project's load from Python, with the GIL released while it runs.

use std::collections::BTreeMap;

use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::{Bound, Py, PyAny, PyResult, pyfunction};
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods};
use serde_json::{Value, json};
use sqlload::adapters::session::models::SessionConnection;
use sqlload::loader::models::PlanOptions;

use crate::bindings::constants::{ADAPTER, DATABASE, DESCRIPTION, OPENER};
use crate::runs::main::run_project::run_project;
use crate::runs::models::ProjectRun;
use crate::sessions::main::session_connection::session_connection;

/// Load `project` (SQLLoad TOML) through `sessions`; `{tables, planned, missing}` JSON, nothing loaded when `require` misses.
#[pyfunction]
#[pyo3(signature = (*, project, sessions, tables, reload = false))]
pub(crate) fn run_sqlload_project(
    project: &str,
    sessions: &Bound<'_, PyDict>,
    tables: (Vec<String>, Vec<String>),
    reload: bool,
) -> PyResult<String> {
    let (select, require): (Vec<String>, Vec<String>) = tables;
    let connections: BTreeMap<String, SessionConnection> = session_connections(sessions)?;
    let options = PlanOptions {
        select,
        reload,
        ..PlanOptions::default()
    };
    let project: String = project.to_owned();
    let run: ProjectRun = sessions
        .py()
        .detach(move || run_project(&project, connections, (options, require)))
        .map_err(PyRuntimeError::new_err)?;
    let rows: Vec<Value> = run
        .tables
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
    Ok(json!({"tables": rows, "planned": run.planned, "missing": run.missing}).to_string())
}

/// Each of `sessions`, `{adapter, database, opener, description}` by name, as SQLLoad's session connection.
pub(crate) fn session_connections(
    sessions: &Bound<'_, PyDict>,
) -> PyResult<BTreeMap<String, SessionConnection>> {
    let mut connections: BTreeMap<String, SessionConnection> = BTreeMap::new();
    for (name, session) in sessions.iter() {
        let session: &Bound<'_, PyDict> = session.cast()?;
        let opener: Py<PyAny> = required(session, OPENER)?.unbind();
        let connection = session_connection(
            &required(session, ADAPTER)?.extract::<String>()?,
            required(session, DATABASE)?.extract()?,
            (opener, required(session, DESCRIPTION)?.extract()?),
        )
        .map_err(PyRuntimeError::new_err)?;
        connections.insert(name.extract()?, connection);
    }
    Ok(connections)
}

fn required<'py>(session: &Bound<'py, PyDict>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    session
        .get_item(key)?
        .ok_or_else(|| PyKeyError::new_err(format!("a load session needs `{key}`")))
}
