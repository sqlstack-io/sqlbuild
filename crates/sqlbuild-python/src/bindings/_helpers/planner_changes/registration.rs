//! Register planner change detection.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::planner_changes::actions;
use crate::bindings::_helpers::planner_changes::detect;
use crate::bindings::_helpers::planner_changes::metadata;
use crate::bindings::_helpers::planner_changes::renames;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    actions::register(module)?;
    detect::register(module)?;
    metadata::register(module)?;
    renames::register(module)
}
