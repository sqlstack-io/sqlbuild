//! Register the shapes functions and classes.

use crate::semantic_shapes::_helpers::shapes::binding_relation_names;
use crate::semantic_shapes::_helpers::shapes::published_model_shape;
use crate::semantic_shapes::_helpers::shapes::semantic_shapes;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the shapes functions and classes.
pub fn register_shapes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(semantic_shapes, module)?)?;
    module.add_function(wrap_pyfunction!(published_model_shape, module)?)?;
    module.add_function(wrap_pyfunction!(binding_relation_names, module)?)
}
