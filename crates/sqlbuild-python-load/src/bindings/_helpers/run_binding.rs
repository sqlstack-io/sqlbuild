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
    let run: ProjectRun = detached_load(
        (project, sessions),
        (tables, reload),
        |project, (connections, options, require)| {
            run_project(
                (&project, Vec::new(), None),
                connections,
                (options, require),
            )
        },
    )?;
    Ok(run_report(run))
}

/// `run` as the JSON report Python reads: `{tables, planned, missing}`, each table's rows or error.
pub(crate) fn run_report(run: ProjectRun) -> String {
    let rows: Vec<Value> = run
        .tables
        .into_iter()
        .map(|table| {
            json!({
                "table": table.table,
                "rows": table.loaded.as_ref().ok(),
                "error": table.loaded.as_ref().err(),
                "changes": table.changes,
                "warnings": table.warnings,
                "retryable": table.retryable,
            })
        })
        .collect();
    json!({"tables": rows, "planned": run.planned, "missing": run.missing}).to_string()
}

/// SQLLoad's connections, plan options and required tables for one plan or run from Python.
pub(crate) type LoadRequest = (
    BTreeMap<String, SessionConnection>,
    PlanOptions,
    Vec<String>,
);

/// `work` on `project` and its load request (`sessions`, `tables`, `reload`), with the GIL released.
pub(crate) fn detached_load<T: Send>(
    (project, sessions): (&str, &Bound<'_, PyDict>),
    (tables, reload): ((Vec<String>, Vec<String>), bool),
    work: impl FnOnce(String, LoadRequest) -> Result<T, String> + Send,
) -> PyResult<T> {
    let (select, require): (Vec<String>, Vec<String>) = tables;
    let options = PlanOptions {
        select,
        reload,
        ..PlanOptions::default()
    };
    let request: LoadRequest = (session_connections(sessions)?, options, require);
    let project: String = project.to_owned();
    sessions
        .py()
        .detach(move || work(project, request))
        .map_err(PyRuntimeError::new_err)
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
