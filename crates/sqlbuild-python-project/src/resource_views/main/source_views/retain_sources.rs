//! Keep `attached` in `owner`; return each source's view.

use crate::compiled_project::models::NativeCompiledProject;
use crate::resource_views::_helpers::source_views::source_record;
use crate::resource_views::models::NativeSourceInput;
use crate::resource_views::types::AttachedSource;
use pyo3::prelude::Bound;

/// Keep `attached` in `owner`; return each source's view.
pub fn retain_sources(
    owner: &Bound<'_, NativeCompiledProject>,
    attached: Vec<AttachedSource>,
) -> Vec<NativeSourceInput> {
    let mut project = owner.borrow_mut();
    let first: usize = project.sources().len();
    let count: usize = attached.len();
    for source in attached {
        project.retain_source(source_record(source));
    }
    drop(project);
    (first..first + count)
        .map(|index| NativeSourceInput::new(owner.clone().unbind(), index))
        .collect()
}
