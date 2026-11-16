//! The definition fingerprint of a function, or the planner input error naming the function.

use crate::planner_identity::_helpers::hashing::fingerprint_error;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::sql_tokens::main::definition_fingerprint::definition_fingerprint;
use sqlbuild_python_base::types::CompilerDetach;

/// The definition fingerprint of a function, or the planner input error naming the function.
pub fn function_definition_hash(
    py: Python<'_>,
    function_name: &str,
    (fingerprint_sql, language, dialect): (&str, &str, Option<&str>),
) -> PyResult<String> {
    py.compiler_detach(|| definition_fingerprint(fingerprint_sql, language, dialect))
        .map_err(|error| fingerprint_error(py, &format!("Function '{function_name}'"), error))
}
