//! The selected run id, or a new `YYYYMMDDTHHMMSSZ_<12 hex>` compile invocation id.

use crate::model_config::_helpers::config::template_data;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The selected run id, or a new `YYYYMMDDTHHMMSSZ_<12 hex>` compile invocation id.
#[inline]
pub fn run_id<'py>(selected_run_id: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    template_data::run_id(selected_run_id)
}
