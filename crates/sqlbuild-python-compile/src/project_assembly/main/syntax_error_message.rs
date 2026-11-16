//! Python's syntax error message for one check, raising what Python's validation raises.

use crate::project_assembly::_helpers::resources::syntax_value_error;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::assembly::project::main::sql_syntax_error::sql_syntax_error as check_syntax_error;
use sqlbuild_analysis::assembly::project::models::{SyntaxCheck, SyntaxMode};
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

/// Python's syntax error message for one check, raising what Python's validation raises.
pub fn syntax_error_message(
    py: Python<'_>,
    check: &SyntaxCheck,
    dialect: &str,
    mode: SyntaxMode,
) -> PyResult<Option<String>> {
    py.compiler_detach(|| Ok(check_syntax_error(check, dialect, mode)))
        .map_err(compiler_error)?
        .map_err(syntax_value_error)
}
