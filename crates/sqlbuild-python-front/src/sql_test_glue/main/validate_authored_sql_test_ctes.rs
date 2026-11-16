//! Reject the first authored test block, then scenario, whose CTE names cannot be read.

use crate::sql_test_glue::_helpers::authored_ctes::AuthoredBody;
use crate::sql_test_glue::_helpers::authored_ctes::check_bodies;
use crate::sqltext::models::LexicalSyntaxInput;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// Reject the first authored test block, then scenario, whose CTE names cannot be read.
#[pyfunction]
pub fn validate_authored_sql_test_ctes(
    py: Python<'_>,
    test_files: Bound<'_, PyAny>,
    scenario_files: Bound<'_, PyAny>,
    syntax: LexicalSyntaxInput,
) -> Result<(), PyErr> {
    let syntax: LexicalSyntax = syntax.into();
    let mut blocks: Vec<AuthoredBody> = Vec::new();
    for test_file in test_files.try_iter()? {
        let test_file: Bound<'_, PyAny> = test_file?;
        for block in native_field(&test_file, "blocks")?.try_iter()? {
            let block: Bound<'_, PyAny> = block?;
            blocks.push(AuthoredBody::read(
                &block,
                &native_field(&block, "sql_body")?,
                &test_file,
            )?);
        }
    }
    check_bodies(py, &blocks, false, &syntax)?;
    let mut scenarios: Vec<AuthoredBody> = Vec::new();
    for scenario_file in scenario_files.try_iter()? {
        let scenario_file: Bound<'_, PyAny> = scenario_file?;
        scenarios.push(AuthoredBody::read(
            &scenario_file,
            &native_field(&scenario_file, "sql_body")?,
            &scenario_file,
        )?);
    }
    check_bodies(py, &scenarios, true, &syntax)
}
