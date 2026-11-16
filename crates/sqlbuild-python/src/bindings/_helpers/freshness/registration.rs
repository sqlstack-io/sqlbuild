//! Register the native `sqb freshness` command, observation, planning and state functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_planner::freshness::main::register_exception_types::register_exception_types;
use sqlbuild_python_planner::freshness::main::register_freshness_observe::register_freshness_observe;
use sqlbuild_python_planner::freshness::main::register_observation::register_observation;
use sqlbuild_python_planner::freshness::main::register_planning::register_planning;
use sqlbuild_python_planner::freshness::main::register_publication::register_publication;
use sqlbuild_python_planner::freshness::main::register_state_io::register_state_io;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_exception_types(module)?;
    register_freshness_observe(module)?;
    register_observation(module)?;
    register_planning(module)?;
    register_publication(module)?;
    register_state_io(module)
}
