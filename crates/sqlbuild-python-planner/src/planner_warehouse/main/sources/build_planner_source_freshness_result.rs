//! The source freshness evaluation of one inspection scope against the warehouse.

use crate::planner_warehouse::_helpers::sources::source_freshness;
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::native_host::models::Dialect;

/// The source freshness evaluation of one inspection scope against the warehouse.
#[inline]
pub fn build_planner_source_freshness_result<'py>(
    (project, scope): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (dialect, reads): (&Bound<'py, Dialect>, &Bound<'py, PlannerReads>),
    relations: &Bound<'py, PyAny>,
    freshness_state_schemas: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    source_freshness::build_planner_source_freshness_result(
        (project, scope),
        (dialect, reads),
        relations,
        freshness_state_schemas,
    )
}
