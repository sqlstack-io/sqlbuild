//! Discover seed CSV files under seeds/.

use crate::discovery::_helpers::records::declaration_records::discovered;
use crate::discovery::_helpers::records::declaration_records::faults;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_discovery::declaration_files::models::CollectionKind;

/// Discover seed CSV files under seeds/.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub fn discover_seed_files<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    discovered(&faults(project_dir, None), CollectionKind::Seeds, false)
}
