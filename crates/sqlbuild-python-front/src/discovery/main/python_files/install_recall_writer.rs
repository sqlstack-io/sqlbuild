//! Install the `RecallWriter` class sink parameters are recognised by, once, at registration.

use crate::discovery::_helpers::python_files::imported_providers::RECALL_WRITER;
use pyo3::prelude::Bound;
use pyo3::types::PyType;

/// Install the `RecallWriter` class sink parameters are recognised by, once, at registration.
pub fn install_recall_writer(writer: &Bound<'_, PyType>) {
    let _ = RECALL_WRITER.get_or_init(writer.py(), || writer.clone().unbind());
}
