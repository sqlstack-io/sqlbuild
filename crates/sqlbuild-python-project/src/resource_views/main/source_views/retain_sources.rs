//! Keep `attached` in `owner`; return each source's view.

use crate::compiled_project::models::NativeCompiledProject;
use crate::resource_views::_helpers::source_views;
use crate::resource_views::models::NativeSourceInput;
use crate::resource_views::types::AttachedSource;
use pyo3::prelude::Bound;

/// Keep `attached` in `owner`; return each source's view.
#[inline]
pub fn retain_sources(
    owner: &Bound<'_, NativeCompiledProject>,
    attached: Vec<AttachedSource>,
) -> Vec<NativeSourceInput> {
    source_views::retain_sources(owner, attached)
}
