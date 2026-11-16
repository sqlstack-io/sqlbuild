//! The latest recorded rename on `relation` naming either column, or `None`.

use crate::state_rows::_helpers::migrations::migration_projections;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The latest recorded rename on `relation` naming either column, or `None`.
#[inline]
pub fn newest_column<'py>(
    events: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
    (origin_column, destination_column): (&str, &str),
) -> PyResult<Bound<'py, PyAny>> {
    migration_projections::newest_column(events, relation, (origin_column, destination_column))
}
