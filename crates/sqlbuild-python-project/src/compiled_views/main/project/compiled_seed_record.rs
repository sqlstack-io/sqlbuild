//! The seed as a `CompiledSeed`, projected from its owner when it is a native view.

use crate::compiled_views::_helpers::project::records::record;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::types::RecordId;

/// The seed as a `CompiledSeed`, projected from its owner when it is a native view.
#[pyfunction]
pub fn compiled_seed_record<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    record(seed, RecordId::CompiledSeed)
}
