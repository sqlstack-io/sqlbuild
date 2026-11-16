//! Return declared dynamic families accepting a physical column name.

use crate::contracts::_helpers::spec_resolution::family_accepts;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyTuple;

/// Return declared dynamic families accepting a physical column name.
#[pyfunction]
#[pyo3(signature = (*, families, column_name))]
pub fn matching_dynamic_families<'py>(
    families: &Bound<'py, PyAny>,
    column_name: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    let mut matching: Vec<Bound<'py, PyAny>> = Vec::new();
    for family in families.try_iter()? {
        let family: Bound<'py, PyAny> = family?;
        if family_accepts(&family, column_name)? {
            matching.push(family);
        }
    }
    PyTuple::new(families.py(), matching)
}
