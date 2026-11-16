//! Create the schema and observations table in `location` if missing, then append `rows`.

use crate::native_host::_helpers::observations_state;
use crate::native_host::models::Location;
use crate::native_host::models::ObservationAppend;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Create the schema and observations table in `location` if missing, then append `rows`.
#[inline]
pub fn append_observations(
    adapter: &Bound<'_, PyAny>,
    connection: &Bound<'_, PyAny>,
    location: &Location,
    rows: &[ObservationAppend],
) -> PyResult<()> {
    observations_state::append_observations(adapter, connection, location, rows)
}
