//! In a delta compile, the document store beside its reuse slots, when it opens.

use crate::compile_context::_helpers::yaml_cache;
use crate::compile_context::models::DocumentStore;

/// In a delta compile, the document store beside its reuse slots, when it opens.
#[inline]
pub fn open_documents(py: pyo3::Python<'_>) -> Option<DocumentStore> {
    yaml_cache::open_documents(py)
}
