//! Changes of stale-warning models whose identities differ from the execution identities.

use crate::planner_phases::_helpers::outputs::identities;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Changes of stale-warning models whose identities differ from the execution identities.
#[inline]
pub fn detect_stale_warning_changes<'py>(
    (project, scopes): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    snapshot: &Bound<'py, PyAny>,
    (identities, execution_changes): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    identities::detect_stale_warning_changes(
        (project, scopes),
        snapshot,
        (identities, execution_changes),
    )
}
