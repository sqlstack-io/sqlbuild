//! Return model closures resolved natively in one project batch, keyed by SQL test key.

use crate::sql_test_glue::_helpers::plan::plan_driver;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Return model closures resolved natively in one project batch, keyed by SQL test key.
#[inline]
pub fn sql_test_model_chain_names_by_key<'py>(
    project: &Bound<'py, PyAny>,
    tests: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    plan_driver::sql_test_model_chain_names_by_key(project, tests)
}
