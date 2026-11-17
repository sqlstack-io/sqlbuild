//! Register the artifact functions and classes.

use crate::dag_artifact::main::build_dag_json::build_dag_json;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the artifact functions and classes.
pub fn register_artifact(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_dag_json, module)?)
}
