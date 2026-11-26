//! The `@loader` runs SQLLoad's engine is loading, by table: what SQLBuild's runtime module calls back into.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};

/// Each running loader's Python run (`EngineLoaderRun`), by the table its engine run reads.
static LOADER_RUNS: LazyLock<Mutex<HashMap<String, Py<PyAny>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Holds `table`'s run for the engine's calls until it is dropped.
pub(crate) struct RegisteredRun {
    table: String,
}

impl Drop for RegisteredRun {
    fn drop(&mut self) {
        if let Ok(mut runs) = LOADER_RUNS.lock() {
            runs.remove(&self.table);
        }
    }
}

/// Keep `run` as the run of `table` while the returned guard lives.
pub(crate) fn register_run(table: &str, run: Py<PyAny>) -> RegisteredRun {
    if let Ok(mut runs) = LOADER_RUNS.lock() {
        runs.insert(table.to_owned(), run);
    }
    RegisteredRun {
        table: table.to_owned(),
    }
}

/// The run of `table`, or a `KeyError` when no loader is loading it.
pub(crate) fn run_of<'py>(py: Python<'py>, table: &str) -> PyResult<Bound<'py, PyAny>> {
    let runs = LOADER_RUNS
        .lock()
        .map_err(|_| PyKeyError::new_err("the loader runs are unavailable"))?;
    runs.get(table)
        .map(|run| run.bind(py).clone())
        .ok_or_else(|| PyKeyError::new_err(format!("no @loader is loading table {table}")))
}
