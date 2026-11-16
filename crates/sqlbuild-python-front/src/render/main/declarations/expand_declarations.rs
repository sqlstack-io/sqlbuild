//! Splice each reference in order, raising the attachment's error for the first bad one.

use crate::render::_helpers::declarations::declaration_expansion;
use crate::render::models::DeclarationExpansion;
use crate::render::models::ExpandedDeclarations;
use pyo3::prelude::PyResult;

/// Splice each reference in order, raising the attachment's error for the first bad one.
#[inline]
pub fn expand_declarations<'py>(
    request: &DeclarationExpansion<'_, 'py>,
) -> PyResult<ExpandedDeclarations<'py>> {
    declaration_expansion::expand_declarations(request)
}
