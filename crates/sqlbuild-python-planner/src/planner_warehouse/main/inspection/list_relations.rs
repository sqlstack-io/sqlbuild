//! The listed relations of a database, narrowed to schemas and names.

use crate::planner_warehouse::_helpers::inspection::snapshot;
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// The listed relations of `database` narrowed to `schemas` and `names`.
#[inline]
pub fn list_relations<'py>(
    reads: &Bound<'py, PlannerReads>,
    database: &Bound<'py, PyAny>,
    schemas: Bound<'py, PyAny>,
    names: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    snapshot::list_relations(reads, database, schemas, names)
}
