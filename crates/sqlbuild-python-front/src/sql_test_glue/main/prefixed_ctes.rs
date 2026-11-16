//! Extract fixture bodies from authored test or scenario CTEs in authored order.

use crate::sql_test_glue::_helpers::authored_ctes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Extract fixture bodies from authored test or scenario CTEs in authored order.
#[inline]
pub fn prefixed_ctes<'py>(ctes: &Bound<'py, PyAny>, prefix: &str) -> PyResult<Bound<'py, PyDict>> {
    authored_ctes::prefixed_ctes(ctes, prefix)
}
