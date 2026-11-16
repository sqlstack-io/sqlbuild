//! Read records, decide `--changed` skips, mint execution IDs and append `started` rows.

use crate::consumption::_helpers::recorder;
use crate::consumption::models::Prepared;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_state::consumption::models::ChangedPolicy;

/// Read records, decide `--changed` skips, mint execution IDs and append `started` rows.
#[inline]
pub fn prepare<'py>(
    (plan, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (adapter, connection_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (policy, full_refresh): (Option<&ChangedPolicy>, bool),
    (run_id, base_on_node_complete): (String, Bound<'py, PyAny>),
) -> PyResult<Prepared<'py>> {
    recorder::prepare(
        (plan, project),
        (adapter, connection_config),
        (policy, full_refresh),
        (run_id, base_on_node_complete),
    )
}
