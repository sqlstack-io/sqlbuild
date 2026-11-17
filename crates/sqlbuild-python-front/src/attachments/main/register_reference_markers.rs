//! Register the reference markers functions and classes.

use crate::attachments::main::assert_no_unresolved_sql_markers::assert_no_unresolved_sql_markers;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the reference markers functions and classes.
pub fn register_reference_markers(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(assert_no_unresolved_sql_markers, module)?)?;
    Ok(())
}
