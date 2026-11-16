//! Parse in-memory SQL test contents into ordered blocks, as test discovery does.

use crate::discovery::_helpers::failures::raised::failure_error;
use crate::discovery::_helpers::records::sql_test_records::options;
use crate::discovery::_helpers::records::sql_test_records::test_blocks;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyStringMethods, PyTuple};
use sqlbuild_discovery::sql_tests::main::parse_sql_test_text::parse_sql_test_text;
use sqlbuild_discovery::sql_tests::models::{DiscoveredSqlTestFile, SqlTestFileOptions};

/// Parse in-memory SQL test contents into ordered blocks, as test discovery does.
#[pyfunction]
#[pyo3(signature = (*, contents, file_path))]
pub fn parse_sql_test_file<'py>(
    contents: String,
    file_path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = file_path.py();
    let path: String = file_path.str()?.to_str()?.to_owned();
    let options: SqlTestFileOptions = options()?;
    let parsed = py.detach(|| parse_sql_test_text(&path, contents, &options));
    let file: DiscoveredSqlTestFile = parsed.map_err(|failure| failure_error(py, failure))?;
    test_blocks(file_path, file.blocks, file.failure)
}
