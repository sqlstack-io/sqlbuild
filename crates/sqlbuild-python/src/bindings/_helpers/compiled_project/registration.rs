use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::{compiled_views, input_loops};

/// Register the compiled project, then its input loops, then its views and records.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sqlbuild_python_project::compiled_project::main::project::register_project::register_project(
        module,
    )?;
    input_loops::registration::register(module)?;
    compiled_views::registration::register(module)?;
    crate::bindings::_helpers::location_command::command::register(module)?;
    sqlbuild_python_project::compiled_project::main::snapshot::register_snapshot::register_snapshot(
        module,
    )
}
