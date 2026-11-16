//! Save the documents this compile loaded; failing to only means parsing them again.

use crate::compile_context::_helpers::yaml_cache;
use crate::compile_context::models::DocumentStore;

/// Save the documents this compile loaded; failing to only means parsing them again.
#[inline]
pub fn save_documents(py: pyo3::Python<'_>, documents: &DocumentStore) {
    yaml_cache::save_documents(py, documents)
}
