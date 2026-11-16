//! Register the fixture context functions and classes.

use crate::planner_strategy::_helpers::fixtures::fixture_context::build_relation_fixture_context;
use crate::planner_strategy::_helpers::fixtures::fixture_context::fixture_planning_work_info;
use crate::planner_strategy::models::RelationFixturePlanningContext;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the fixture context functions and classes.
pub fn register_fixture_context(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<RelationFixturePlanningContext>()?;
    module.add_function(wrap_pyfunction!(build_relation_fixture_context, module)?)?;
    module.add_function(wrap_pyfunction!(fixture_planning_work_info, module)?)
}
