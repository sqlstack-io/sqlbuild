//! Record successful incremental models' cursor `MAX` and the source columns planning described; failures only warn.

use crate::observations::_helpers::publication;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;

/// Record successful incremental models' cursor `MAX` and the source columns planning described; failures only warn.
#[inline]
pub fn append_build_observations(
    (project, successful_models): (&Bound<'_, PyAny>, &[String]),
    (adapter, connection_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    run_id: &str,
) -> Vec<String> {
    publication::append_build_observations(
        (project, successful_models),
        (adapter, connection_config),
        run_id,
    )
}
