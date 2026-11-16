//! The latest recorded move naming `relation` as origin or destination, or `None`.

use crate::state_rows::_helpers::migrations::migration_projections;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The latest recorded move naming `relation` as origin or destination, or `None`.
#[inline]
pub fn newest_move<'py>(
    events: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    migration_projections::newest_move(events, relation)
}
