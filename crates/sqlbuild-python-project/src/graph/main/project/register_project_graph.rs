//! Register the project graph functions and classes.

use crate::graph::models::CompiledProjectGraph;
use crate::graph::models::NativeProjectGraph;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the project graph functions and classes.
pub fn register_project_graph(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeProjectGraph>()?;
    module.add_class::<CompiledProjectGraph>()
}
