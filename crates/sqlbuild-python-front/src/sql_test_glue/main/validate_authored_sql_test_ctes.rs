//! Reject the first authored test block, then scenario, whose CTE names cannot be read.

use crate::sql_test_glue::_helpers::authored_ctes;
use crate::sqltext::models::LexicalSyntaxInput;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// Reject the first authored test block, then scenario, whose CTE names cannot be read.
#[inline]
pub fn validate_authored_sql_test_ctes(
    py: Python<'_>,
    test_files: Bound<'_, PyAny>,
    scenario_files: Bound<'_, PyAny>,
    syntax: LexicalSyntaxInput,
) -> Result<(), PyErr> {
    authored_ctes::validate_authored_sql_test_ctes(py, test_files, scenario_files, syntax)
}
