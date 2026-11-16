//! Register the selection functions and classes.

use crate::dag_artifact::_helpers::selection::load_sqlbuild_dag;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the selection functions and classes.
pub fn register_selection(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(load_sqlbuild_dag, module)?)?;
    Ok(())
}
