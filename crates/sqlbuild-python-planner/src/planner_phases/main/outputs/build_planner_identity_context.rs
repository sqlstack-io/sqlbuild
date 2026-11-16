//! Build planner identity context.

use crate::planner_phases::_helpers::outputs::identities;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Build planner identity context.
#[inline]
pub fn build_planner_identity_context<'py>(
    project: &Bound<'py, PyAny>,
    (scopes, snapshot): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    include_stale_warning_identities: bool,
) -> PyResult<Bound<'py, PyAny>> {
    identities::build_planner_identity_context(
        project,
        (scopes, snapshot),
        include_stale_warning_identities,
    )
}
