//! The source freshness evaluation of one inspection scope against the warehouse.

use crate::planner_warehouse::_helpers::sources::source_freshness;
<<<<<<< HEAD
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::native_host::models::Dialect;
=======
use pyo3::prelude::{Bound, PyAny, PyResult};
>>>>>>> origin/main

/// The source freshness evaluation of one inspection scope against the warehouse.
#[inline]
pub fn build_planner_source_freshness_result<'py>(
    (project, scope): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
<<<<<<< HEAD
    (dialect, reads): (&Bound<'py, Dialect>, &Bound<'py, PlannerReads>),
=======
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
>>>>>>> origin/main
    relations: &Bound<'py, PyAny>,
    freshness_state_schemas: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    source_freshness::build_planner_source_freshness_result(
        (project, scope),
<<<<<<< HEAD
        (dialect, reads),
=======
        (adapter, connection),
>>>>>>> origin/main
        relations,
        freshness_state_schemas,
    )
}
