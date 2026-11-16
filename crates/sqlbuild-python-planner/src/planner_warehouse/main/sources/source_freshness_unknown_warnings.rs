//! One warning per reason that left configured source freshness unknown.

use crate::planner_warehouse::_helpers::sources::source_freshness;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// One warning per reason that left configured source freshness unknown.
#[inline]
pub fn source_freshness_unknown_warnings<'py>(
    source_freshness: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    source_freshness::source_freshness_unknown_warnings(source_freshness)
}
