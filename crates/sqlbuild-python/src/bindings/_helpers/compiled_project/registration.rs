use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::{compiled_views, input_loops};

/// Register the compiled project, then its input loops, then its views and records.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    crate::bindings::_helpers::compiled_project::project::register(module)?;
    input_loops::registration::register(module)?;
    compiled_views::registration::register(module)?;
    crate::bindings::_helpers::location_command::command::register(module)?;
    crate::bindings::_helpers::compiled_project::snapshot::register(module)
}
