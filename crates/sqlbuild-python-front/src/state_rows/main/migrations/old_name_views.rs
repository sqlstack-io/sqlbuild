//! One history per recorded move whose old name needs a compatibility view, in move order.

use crate::state_rows::_helpers::migrations::migration_projections;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// One history per recorded move whose old name needs a compatibility view, in move order.
#[inline]
pub fn old_name_views<'py>(
    moves: &Bound<'py, PyAny>,
    facts: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    migration_projections::old_name_views(moves, facts)
}
