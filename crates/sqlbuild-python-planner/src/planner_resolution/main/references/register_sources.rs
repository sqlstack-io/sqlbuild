//! Register the sources functions and classes.

use crate::planner_resolution::_helpers::references::sources::render_source_read_relation;
use crate::planner_resolution::_helpers::references::sources::resolve_source_references;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the sources functions and classes.
pub fn register_sources(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_source_references, module)?)?;
    module.add_function(wrap_pyfunction!(render_source_read_relation, module)?)?;
    Ok(())
}
