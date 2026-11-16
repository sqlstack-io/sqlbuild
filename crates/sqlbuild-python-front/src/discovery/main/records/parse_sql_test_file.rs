//! Parse in-memory SQL test contents into ordered blocks, as test discovery does.

use crate::discovery::_helpers::records::sql_test_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Parse in-memory SQL test contents into ordered blocks, as test discovery does.
#[inline]
pub fn parse_sql_test_file<'py>(
    contents: String,
    file_path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    sql_test_records::parse_sql_test_file(contents, file_path)
}
