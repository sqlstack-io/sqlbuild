//! `run_sqlload_loader`: one `@loader`'s run on SQLLoad's engine, its Python table called through SQLBuild's runtime module.

use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, pyfunction};
use pyo3::types::PyDict;
use sqlload::loader::models::{PlanOptions, TypedCursors};
use sqlload::python_sources::models::PythonSource;
use sqlload::python_sources::types::PythonHost;

use crate::bindings::_helpers::run_binding::{run_report, session_connections};
use crate::runs::main::run_project::run_project;
use crate::runs::models::LoaderHost;

/// Load `loader` (an `EngineLoaderRun`: its project text, Python source, reload and cursor window) through `sessions`; the report `run_sqlload_project` gives.
#[pyfunction]
#[pyo3(signature = (*, loader, sessions))]
pub(crate) fn run_sqlload_loader(
    loader: &Bound<'_, PyAny>,
    sessions: &Bound<'_, PyDict>,
) -> PyResult<String> {
    let text = |name: &str| -> PyResult<Option<String>> { loader.getattr(name)?.extract() };
    let source: PythonSource =
        serde_json::from_str(&loader.getattr("source")?.extract::<String>()?).map_err(|error| {
            PyValueError::new_err(format!("cannot read the loader's Python source: {error}"))
        })?;
    let options = PlanOptions {
        reload: loader.getattr("reload")?.extract()?,
        typed_cursors: TypedCursors {
            start_ts: text("start_cursor_ts")?,
            end_ts: text("end_cursor_ts")?,
            start_int: text("start_cursor_int")?,
            end_int: text("end_cursor_int")?,
        },
        ..PlanOptions::default()
    };
    let project: String = loader.getattr("project")?.extract()?;
    let connections = session_connections(sessions)?;
    let host: Arc<dyn PythonHost> = Arc::new(LoaderHost {
        run: loader.clone().unbind(),
    });
    let run = loader
        .py()
        .detach(move || {
            run_project(
                (&project, vec![source], Some(host)),
                connections,
                (options, Vec::new()),
            )
        })
        .map_err(PyRuntimeError::new_err)?;
    Ok(run_report(run))
}
