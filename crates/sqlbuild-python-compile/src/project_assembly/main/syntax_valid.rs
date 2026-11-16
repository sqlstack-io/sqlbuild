//! Whether every check parses, raising what Python's validation raises.

use crate::project_assembly::_helpers::resources::SYNTAX_CHECK_CONTEXT;
use crate::project_assembly::_helpers::resources::syntax_value_error;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::assembly::project::main::check_sql_syntax::check_sql_syntax;
use sqlbuild_analysis::assembly::project::models::SyntaxCheck;
use sqlbuild_core::panics::main::native_failure::native_failure;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

/// Whether every check parses, raising what Python's validation raises.
pub fn syntax_valid(py: Python<'_>, dialect: &str, checks: &[SyntaxCheck]) -> PyResult<bool> {
    py.compiler_detach(|| check_sql_syntax(dialect, checks))
        .map_err(|reason| compiler_error(native_failure(SYNTAX_CHECK_CONTEXT, &reason)))?
        .map_err(syntax_value_error)
}
