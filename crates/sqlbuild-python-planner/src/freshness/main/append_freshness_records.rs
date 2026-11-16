//! Append observed source freshness only after the affected selected models succeed.

use crate::freshness::_helpers::publication;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Append observed source freshness only after the affected selected models succeed.
#[inline]
pub fn append_freshness_records<'py>(
    (plan, successful_models): (&Bound<'py, PyAny>, &[String]),
    adapter: &Bound<'py, PyAny>,
    connection_config: &Bound<'py, PyAny>,
    run_id: &Bound<'py, PyAny>,
) -> PyResult<()> {
    publication::append_freshness_records(
        (plan, successful_models),
        adapter,
        connection_config,
        run_id,
    )
}
