//! Register the validation functions and classes.

use crate::lifecycle_events::_helpers::validation::frozen_json;
use crate::lifecycle_events::_helpers::validation::optional_text_check;
use crate::lifecycle_events::_helpers::validation::required_text_check;
use crate::lifecycle_events::_helpers::validation::schema_version_check;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the validation functions and classes.
pub fn register_validation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(schema_version_check, module)?)?;
    module.add_function(wrap_pyfunction!(frozen_json, module)?)?;
    module.add_function(wrap_pyfunction!(required_text_check, module)?)?;
    module.add_function(wrap_pyfunction!(optional_text_check, module)?)?;
    Ok(())
}
