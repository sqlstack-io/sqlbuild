//! Register the source relations functions and classes.

use crate::references::main::render_source_relation::render_source_relation;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the source relations functions and classes.
pub fn register_source_relations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_source_relation, module)?)?;
    Ok(())
}
