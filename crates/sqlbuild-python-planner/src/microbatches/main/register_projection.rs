//! Register the projection functions and classes.

use crate::microbatches::_helpers::projection::coverage;
use crate::microbatches::_helpers::projection::latest;
use crate::microbatches::_helpers::projection::replay;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the projection functions and classes.
pub fn register_projection(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(coverage, module)?)?;
    module.add_function(wrap_pyfunction!(replay, module)?)?;
    module.add_function(wrap_pyfunction!(latest, module)?)
}
