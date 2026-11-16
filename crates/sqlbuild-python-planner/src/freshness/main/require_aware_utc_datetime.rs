//! An aware freshness `datetime` in UTC; a naive value is an observation error.

use crate::freshness::_helpers::observation::observation_error;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_front::state_rows::main::event_rows::in_utc::in_utc;
use sqlbuild_python_front::state_rows::main::event_rows::offset_aware::offset_aware;

/// An aware freshness `datetime` in UTC; a naive value is an observation error.
pub fn require_aware_utc_datetime<'py>(
    value: &Bound<'py, PyAny>,
    field_name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if !offset_aware(value)? {
        return Err(observation_error(
            value.py(),
            &format!("source freshness {field_name} must be timezone-aware"),
        ));
    }
    in_utc(value)
}
