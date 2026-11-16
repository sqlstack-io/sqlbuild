//! Note unselected upstreams deferred to pre-migration names, and each lookup in `failures`.

use crate::planner_phases::_helpers::outputs::output_assembly;
use crate::planner_phases::models::PlanOutputFields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Note unselected upstreams deferred to pre-migration names, and each lookup in `failures`.
#[inline]
pub fn add_deferred_origin_notes<'py>(
    output: PlanOutputFields<'py>,
    (project, scopes): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (deferral, failures): (&Bound<'py, PyAny>, &[(String, String)]),
) -> PyResult<PlanOutputFields<'py>> {
    output_assembly::add_deferred_origin_notes(output, (project, scopes), (deferral, failures))
}
