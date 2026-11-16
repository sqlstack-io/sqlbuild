//! Project selected direct seed work and canonical identities.

use crate::planner_entries::_helpers::plans::command_plans;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Project selected direct seed work and canonical identities.
#[inline]
pub fn build_seed_command_plan<'py>(
    project: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
    fingerprints: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    command_plans::build_seed_command_plan(project, scope, relations, fingerprints)
}
