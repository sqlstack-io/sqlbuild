//! Register the syntax validation functions and classes.

use crate::project_assembly::_helpers::syntax_validation::hook_sql_statements;
use crate::project_assembly::_helpers::syntax_validation::opt_out_help;
use crate::project_assembly::main::validate_hook_sql_syntax::validate_hook_sql_syntax;
use crate::project_assembly::main::validate_sql_syntax::validate_sql_syntax;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the syntax validation functions and classes.
pub fn register_syntax_validation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(opt_out_help, module)?)?;
    module.add_function(wrap_pyfunction!(validate_hook_sql_syntax, module)?)?;
    module.add_function(wrap_pyfunction!(validate_sql_syntax, module)?)?;
    module.add_function(wrap_pyfunction!(hook_sql_statements, module)?)
}
