//! Direct model identities of `(functions, scope)`; unchanged nodes keep their `recorded` fingerprints.

use crate::planner_identity::_helpers::identities;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Direct model identities of `(functions, scope)`; unchanged nodes keep their `recorded` fingerprints.
#[inline]
pub fn direct_model_version_identities<'py>(
    (functions, scope): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    dialect: Option<String>,
    (seeds, source_hashes, hooks): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    recorded: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    identities::direct_model_version_identities(
        (functions, scope),
        dialect,
        (seeds, source_hashes, hooks),
        recorded,
    )
}
