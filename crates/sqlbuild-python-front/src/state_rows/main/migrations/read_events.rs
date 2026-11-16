//! Every event of `layout`'s table this version reads from one schema, from `stored` columns when known.

use crate::state_rows::_helpers::migrations::migration_io;
use crate::state_rows::models::EventTableLayout;
use crate::state_rows::models::StateAccess;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Every event of `layout`'s table this version reads from one schema, from `stored` columns when known.
#[inline]
pub fn read_events<'py>(
    access: &StateAccess<'py>,
    layout: &EventTableLayout,
    (database, schema): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    stored: Option<&[String]>,
) -> PyResult<Bound<'py, PyTuple>> {
    migration_io::read_events(access, layout, (database, schema), stored)
}
