//! Resolve relation locations and source metadata for plan entry construction.

use crate::planner_entries::_helpers::entries::relations;
use crate::planner_entries::models::RelationsRequest;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Resolve relation locations and source metadata for plan entry construction.
#[inline]
pub fn planner_relations_context<'py>(
    request: RelationsRequest<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    relations::planner_relations_context(request)
}
