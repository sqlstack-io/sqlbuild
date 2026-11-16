//! Hold an observation's `observed_at` and datetime `data_version` as aware UTC values.

use crate::freshness::main::require_aware_utc_datetime::require_aware_utc_datetime;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDateTime;
use sqlbuild_python_base::records::constants::DATA_VERSION_FIELD;
use sqlbuild_python_base::records::constants::OBSERVED_AT_FIELD;

/// Hold an observation's `observed_at` and datetime `data_version` as aware UTC values.
pub fn normalize_observation(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let observed_at: Bound<'_, PyAny> =
        require_aware_utc_datetime(values[OBSERVED_AT_FIELD].bind(py), "observed_at")?;
    values[OBSERVED_AT_FIELD] = observed_at.unbind();
    let data_version: &Bound<'_, PyAny> = values[DATA_VERSION_FIELD].bind(py);
    if data_version.is_instance_of::<PyDateTime>() {
        let data_version: Bound<'_, PyAny> =
            require_aware_utc_datetime(data_version, "data_version")?;
        values[DATA_VERSION_FIELD] = data_version.unbind();
    }
    Ok(values)
}
