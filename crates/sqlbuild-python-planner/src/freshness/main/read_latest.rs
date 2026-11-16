//! Latest records of one schema, trusting the caller-resolved table existence.

use crate::freshness::_helpers::state_io;
use crate::freshness::models::StateRead;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Latest records of one schema, trusting the caller-resolved table existence.
#[inline]
pub fn read_latest<'py>(
    read: &StateRead<'py>,
    (database, schema): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    table_exists: bool,
    source_names: Option<&[String]>,
) -> PyResult<Bound<'py, PyAny>> {
    state_io::read_latest(read, (database, schema), table_exists, source_names)
}
