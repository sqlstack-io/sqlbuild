//! The selected run id, or a new `YYYYMMDDTHHMMSSZ_<12 hex>` compile invocation id.

use crate::model_config::_helpers::config::template_data::RUN_ID_SUFFIX_LENGTH;
use crate::model_config::_helpers::config::template_data::TIMESTAMP_SEPARATORS;
use crate::model_config::_helpers::config::template_data::UUID_MODULE;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_core::scenario_command::main::format_captured_at::format_captured_at;
use sqlbuild_python_base::boundary::main::imported::imported;
use std::time::{SystemTime, UNIX_EPOCH};

/// The selected run id, or a new `YYYYMMDDTHHMMSSZ_<12 hex>` compile invocation id.
pub fn run_id<'py>(selected_run_id: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = selected_run_id.py();
    if !selected_run_id.is_none() {
        return Ok(selected_run_id.clone());
    }
    let epoch_seconds: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        });
    let timestamp: String = format_captured_at(epoch_seconds).replace(TIMESTAMP_SEPARATORS, "");
    let suffix: String = imported(py, UUID_MODULE, "uuid4")?
        .call0()?
        .getattr("hex")?
        .extract()?;
    let suffix: &str = suffix.get(..RUN_ID_SUFFIX_LENGTH).unwrap_or(&suffix);
    Ok(PyString::new(py, &format!("{timestamp}_{suffix}")).into_any())
}
