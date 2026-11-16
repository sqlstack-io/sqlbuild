//! Install the `RecallWriter` class sink parameters are recognised by, once, at registration.

use crate::discovery::_helpers::python_files::imported_providers;
use pyo3::prelude::Bound;
use pyo3::types::PyType;

/// Install the `RecallWriter` class sink parameters are recognised by, once, at registration.
#[inline]
pub fn install_recall_writer(writer: &Bound<'_, PyType>) {
    imported_providers::install_recall_writer(writer)
}
