//! Fixture SQL with its untyped null columns cast to the expected types.

use crate::planner_strategy::_helpers::fixtures::fixture_sql::CompletionSql;
use crate::planner_strategy::_helpers::fixtures::fixture_sql::render_completed;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
<<<<<<< HEAD
use sqlbuild_python_base::native_host::models::Dialect;
=======
>>>>>>> origin/main
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Fixture SQL with its untyped null columns cast to the expected types.
pub fn complete_typed_null_columns(
    sql: &str,
    inference: &Bound<'_, PyAny>,
    expected_types: &[(String, String)],
<<<<<<< HEAD
    dialect: &Bound<'_, Dialect>,
=======
    adapter: &Bound<'_, PyAny>,
>>>>>>> origin/main
) -> PyResult<String> {
    let py = inference.py();
    let typed_nulls = PyDict::new(py);
    for name in native_field(inference, "null_literal_names")?.try_iter()? {
        let name = name?;
        let folded: String = name.extract()?;
        if let Some((_, expected)) = expected_types.iter().find(|(column, _)| *column == folded) {
            typed_nulls.set_item(name, expected)?;
        }
    }
    if typed_nulls.is_empty() {
        return Ok(sql.into());
    }
    render_completed(
        CompletionSql {
            sql,
            inferred: native_field(inference, "columns")?,
            typed_nulls,
            quoted_names: native_field(inference, "quoted_names")?,
            completed: Vec::new(),
        },
<<<<<<< HEAD
        dialect,
=======
        adapter,
>>>>>>> origin/main
    )
}
