//! The layout of the state table `kind` names: `model`, `column` or `old_name`.

use crate::state_rows::_helpers::migrations::migration_objects;
use crate::state_rows::models::EventTableLayout;
use pyo3::prelude::PyResult;

/// The layout of the state table `kind` names: `model`, `column` or `old_name`.
#[inline]
pub fn layout(kind: &str) -> PyResult<EventTableLayout> {
    migration_objects::layout(kind)
}
