//! Register the fixture columns functions and classes.

use crate::analysis_session::_helpers::fixture_columns::fixture_inference_count;
use crate::analysis_session::_helpers::fixture_columns::infer_fixture_column_facts;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the fixture columns functions and classes.
pub fn register_fixture_columns(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(infer_fixture_column_facts, module)?)?;
    module.add_function(wrap_pyfunction!(fixture_inference_count, module)?)
}
