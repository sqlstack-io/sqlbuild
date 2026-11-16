//! Resolve scenario config, allowing local overrides to replace project rules.

use crate::contracts::_helpers::spec_resolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Resolve scenario config, allowing local overrides to replace project rules.
#[inline]
pub fn resolve_effective_scenario_config<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    spec_resolution::resolve_effective_scenario_config(project_config, local_config)
}
