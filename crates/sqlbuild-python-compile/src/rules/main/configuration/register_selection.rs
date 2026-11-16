//! Register the selection functions and classes.

use crate::rules::_helpers::configuration::selection::rule_selected;
use crate::rules::_helpers::configuration::selection::selected_rule_codes;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the selection functions and classes.
pub fn register_selection(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(selected_rule_codes, module)?)?;
    module.add_function(wrap_pyfunction!(rule_selected, module)?)
}
