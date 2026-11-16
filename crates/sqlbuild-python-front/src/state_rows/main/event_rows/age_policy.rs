//! Optional timestamp age policy for one source freshness observation.

use crate::state_rows::_helpers::event_rows::freshness;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Optional timestamp age policy for one source freshness observation.
#[inline]
pub fn age_policy<'py>(
    policy: &Bound<'py, PyAny>,
    data_version: &Bound<'py, PyAny>,
    observed_at: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    freshness::age_policy(policy, data_version, observed_at)
}
