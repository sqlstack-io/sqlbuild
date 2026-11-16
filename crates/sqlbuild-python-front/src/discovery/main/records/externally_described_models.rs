//! Model files that `[defaults]`, `[path_defaults]` or a model schema describe.

use crate::discovery::_helpers::records::description_inputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyFrozenSet;

/// Model files that `[defaults]`, `[path_defaults]` or a model schema describe.
#[inline]
pub fn externally_described_models<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    description_inputs::externally_described_models(project_dir)
}
