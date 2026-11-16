//! Hold an observation's `observed_at` and datetime `data_version` as aware UTC values.

use crate::freshness::_helpers::observation;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Hold an observation's `observed_at` and datetime `data_version` as aware UTC values.
#[inline]
pub fn normalize_observation(py: Python<'_>, values: Vec<Py<PyAny>>) -> PyResult<Vec<Py<PyAny>>> {
    observation::normalize_observation(py, values)
}
