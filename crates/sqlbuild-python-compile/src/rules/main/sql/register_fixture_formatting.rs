//! Register the fixture formatting functions and classes.

use crate::rules::main::sql::format_redundant_fixture_nulls::format_redundant_fixture_nulls;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the fixture formatting functions and classes.
pub fn register_fixture_formatting(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_redundant_fixture_nulls, module)?)
}
