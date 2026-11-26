//! SQLBuild's runtime module for SQLLoad's Python host: the functions its engine calls to run a `@loader`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use serde_json::Value;

use crate::bindings::_helpers::loader_runs::run_of;
use crate::bindings::constants::{RENAME_PROBLEM, RUNTIME_MODULE, RUNTIME_SUBMODULE};

/// No source is found by importing: SQLBuild declares each loader's Python source itself.
#[pyfunction]
fn discover(root: &str, python_paths: Vec<String>) -> String {
    let _ = (root, python_paths);
    "[]".to_owned()
}

/// What the loader the read `request` (a `TableRead`, as JSON) names returns or yields, through its run.
#[pyfunction]
fn start_table<'py>(py: Python<'py>, request: &str) -> PyResult<Bound<'py, PyAny>> {
    let read: Value = serde_json::from_str(request)
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
    let table: &str = read
        .get("table")
        .and_then(Value::as_str)
        .unwrap_or_default();
    run_of(py, table)?.call_method1("table_items", (request,))
}

/// The logged position the rows crossing now commit with.
#[pyfunction]
fn current_state(py: Python<'_>, table: &str) -> PyResult<Option<String>> {
    run_of(py, table)?.call_method0("current_state")?.extract()
}

/// The position the loader left in `ctx.state`, once it finished.
#[pyfunction]
fn table_state(py: Python<'_>, table: &str) -> PyResult<Option<String>> {
    run_of(py, table)?.call_method0("final_state")?.extract()
}

/// A loader has no rename functions.
#[pyfunction]
fn rename(name: &str, columns: Option<&str>, context: &str) -> (Option<String>, Option<String>) {
    let _ = (name, columns, context);
    (None, Some(RENAME_PROBLEM.to_owned()))
}

/// Nothing to close: SQLBuild's runs own their connections and clients.
#[pyfunction]
fn finish_run() {}

/// Add the runtime module to `module` and to `sys.modules` under the name the engine imports.
pub(crate) fn register_runtime(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py: Python<'_> = module.py();
    let runtime: Bound<'_, PyModule> = PyModule::new(py, RUNTIME_SUBMODULE)?;
    runtime.add_function(wrap_pyfunction!(discover, &runtime)?)?;
    runtime.add_function(wrap_pyfunction!(start_table, &runtime)?)?;
    runtime.add_function(wrap_pyfunction!(current_state, &runtime)?)?;
    runtime.add_function(wrap_pyfunction!(table_state, &runtime)?)?;
    runtime.add_function(wrap_pyfunction!(rename, &runtime)?)?;
    runtime.add_function(wrap_pyfunction!(finish_run, &runtime)?)?;
    module.add_submodule(&runtime)?;
    py.import("sys")?
        .getattr("modules")?
        .cast_into::<PyDict>()?
        .set_item(RUNTIME_MODULE, runtime)
}
