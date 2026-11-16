//! Resource assembly facts for one project, computed natively.

use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyString, PyStringMethods};
use pyo3::{PyErr, pyfunction, wrap_pyfunction};
use sqlbuild_analysis::assembly::project::main::check_sql_syntax::check_sql_syntax;
use sqlbuild_analysis::assembly::project::main::sql_syntax_error::sql_syntax_error as check_syntax_error;
use sqlbuild_analysis::assembly::project::models::{SyntaxCheck, SyntaxFailure, SyntaxMode};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_core::panics::main::native_failure::native_failure;

use crate::bindings::_helpers::input_loops::model_loop_prepass::pooled;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

const SYNTAX_CHECK_CONTEXT: &str = "native SQL syntax check";

/// Python's syntax error message for one SQL string, None where it parses.
#[pyfunction]
#[pyo3(signature = (sql, placeholders, dialect, parse_one=false))]
fn sql_syntax_error(
    sql: &Bound<'_, PyString>,
    placeholders: Option<HashMap<String, String>>,
    dialect: Option<String>,
    parse_one: bool,
) -> PyResult<Option<String>> {
    let py: Python<'_> = sql.py();
    let check = SyntaxCheck {
        sql: sql.to_str()?.to_owned(),
        placeholders: placeholders.unwrap_or_default().into_iter().collect(),
    };
    let dialect: String = dialect
        .filter(|dialect| !dialect.is_empty())
        .unwrap_or_else(|| "generic".to_owned());
    let mode = if parse_one {
        SyntaxMode::Parse
    } else {
        SyntaxMode::Validate
    };
    syntax_error_message(py, &check, &dialect, mode)
}

/// Python's syntax error message for one check, raising what Python's validation raises.
pub(crate) fn syntax_error_message(
    py: Python<'_>,
    check: &SyntaxCheck,
    dialect: &str,
    mode: SyntaxMode,
) -> PyResult<Option<String>> {
    py.compiler_detach(|| Ok(check_syntax_error(check, dialect, mode)))
        .map_err(compiler_error)?
        .map_err(syntax_value_error)
}

/// Each check's `syntax_error_message`, in order, parsed together on the compile pool.
pub(crate) fn syntax_error_messages(
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

/// The `ValueError` Python's syntax validation raises before parsing.
fn syntax_value_error(failure: SyntaxFailure) -> PyErr {
    match failure {
        SyntaxFailure::Normalization(message) => PyValueError::new_err(message),
        SyntaxFailure::UnknownDialect(name) => {
            PyValueError::new_err(format!("Unknown dialect: {name}"))
        }
    }
}

/// Whether every check parses, raising what Python's validation raises.
pub(crate) fn syntax_valid(
    py: Python<'_>,
    dialect: &str,
    checks: &[SyntaxCheck],
) -> PyResult<bool> {
    py.compiler_detach(|| check_sql_syntax(dialect, checks))
        .map_err(|reason| compiler_error(native_failure(SYNTAX_CHECK_CONTEXT, &reason)))?
        .map_err(syntax_value_error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(sql_syntax_error, module)?)?;
    Ok(())
}
