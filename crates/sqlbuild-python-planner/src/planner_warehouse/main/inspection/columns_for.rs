//! The columns of listed relations.

use crate::planner_warehouse::_helpers::inspection::snapshot;
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// The columns of each listed relation, keyed by identity.
#[inline]
pub fn columns_for<'py>(
    reads: &Bound<'py, PlannerReads>,
    relations: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    snapshot::columns_for(reads, relations)
}
