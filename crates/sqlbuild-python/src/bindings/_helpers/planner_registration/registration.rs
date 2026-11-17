//! Registers every planner binding group.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::planner_changes;
use crate::bindings::_helpers::planner_entries;
use crate::bindings::_helpers::planner_identity;
use crate::bindings::_helpers::planner_migrations;
use crate::bindings::_helpers::planner_phases;
use crate::bindings::_helpers::planner_resolution;
use crate::bindings::_helpers::planner_scenario;
use crate::bindings::_helpers::planner_scope;
use crate::bindings::_helpers::planner_selection;
use crate::bindings::_helpers::planner_strategy;
use crate::bindings::_helpers::planner_warehouse;
use crate::bindings::_helpers::relation_naming;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    planner_changes::registration::register(module)?;
    planner_migrations::registration::register(module)?;
    planner_phases::registration::register(module)?;
    planner_warehouse::registration::register(module)?;
    planner_identity::registration::register(module)?;
    planner_selection::registration::register(module)?;
    planner_scope::registration::register(module)?;
    planner_scenario::registration::register(module)?;
    planner_strategy::registration::register(module)?;
    planner_resolution::registration::register(module)?;
    planner_entries::registration::register(module)?;
    relation_naming::registration::register(module)?;
    Ok(())
}
