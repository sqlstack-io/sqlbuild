//! Pair each declaration with the CSV file of its name, raising for the first without one.

use crate::compiled_views::_helpers::seed_views;
use crate::compiled_views::types::SeedRecord;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Pair each declaration with the CSV file of its name, raising for the first without one.
#[inline]
pub fn paired_seeds(
    declarations: Vec<(Py<PyAny>, Py<PyAny>)>,
    seed_files: &[Py<PyAny>],
    compile_error: &Bound<'_, PyAny>,
) -> PyResult<Vec<SeedRecord>> {
    seed_views::paired_seeds(declarations, seed_files, compile_error)
}
