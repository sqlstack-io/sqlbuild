//! What `plan --changed` previews: what a build would run now, read without recording anything.

use crate::consumption::_helpers::recorder;
use crate::consumption::models::ChangedDecision;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_state::consumption::models::ChangedPolicy;

/// What `plan --changed` previews: what a build would run now, read without recording anything.
#[inline]
pub fn preview_changed(
    (plan, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (adapter, connection_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (policy, full_refresh): (&ChangedPolicy, bool),
) -> PyResult<(ChangedDecision, Vec<String>)> {
    recorder::preview_changed(
        (plan, project),
        (adapter, connection_config),
        (policy, full_refresh),
    )
}
