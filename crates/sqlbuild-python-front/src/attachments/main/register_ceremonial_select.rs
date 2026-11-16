//! Register the ceremonial select functions and classes.

use crate::attachments::_helpers::ceremonial_select::omitted_ceremonial_select;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the ceremonial select functions and classes.
pub fn register_ceremonial_select(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(omitted_ceremonial_select, module)?)?;
    Ok(())
}
