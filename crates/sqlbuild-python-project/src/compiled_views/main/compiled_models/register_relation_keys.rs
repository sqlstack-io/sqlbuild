//! Register the relation keys functions and classes.

use crate::compiled_views::_helpers::compiled_models::relation_keys::compiled_relation_keys;
use crate::compiled_views::_helpers::compiled_models::relation_keys::declared_relation_keys;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the relation keys functions and classes.
pub fn register_relation_keys(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(declared_relation_keys, module)?)?;
    module.add_function(wrap_pyfunction!(compiled_relation_keys, module)?)
}
