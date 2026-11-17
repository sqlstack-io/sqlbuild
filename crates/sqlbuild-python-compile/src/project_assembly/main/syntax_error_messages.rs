//! Each check's `syntax_error_message`, in order, parsed together on the compile pool.

use crate::project_assembly::_helpers::resources::syntax_value_error;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::assembly::project::main::sql_syntax_error::sql_syntax_error as check_syntax_error;
use sqlbuild_analysis::assembly::project::models::{SyntaxCheck, SyntaxMode};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::native_threads::main::pooled::pooled;

/// Each check's `syntax_error_message`, in order, parsed together on the compile pool.
pub fn syntax_error_messages(
    py: Python<'_>,
    checks: &[SyntaxCheck],
    dialect: &str,
    mode: SyntaxMode,
) -> Vec<PyResult<Option<String>>> {
    py.detach(|| {
        pooled(checks, |check| {
            catch_compiler_panic(|| Ok(check_syntax_error(check, dialect, mode)))
        })
    })
    .into_iter()
    .map(|outcome| outcome.map_err(compiler_error)?.map_err(syntax_value_error))
    .collect()
}
