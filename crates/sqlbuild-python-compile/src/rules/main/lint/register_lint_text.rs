//! Register the lint text functions and classes.

use crate::rules::_helpers::lint::lint_text::format_lint_sql_bodies;
use crate::rules::_helpers::lint::lint_text::named_declaration_role_kind;
use crate::rules::_helpers::lint::lint_text::neutralize_lint_interpolation;
use crate::rules::_helpers::lint::lint_text::reject_lint_suppressions;
use crate::rules::_helpers::lint::lint_text::restore_lint_interpolation;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the lint text functions and classes.
pub fn register_lint_text(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(neutralize_lint_interpolation, module)?)?;
    module.add_function(wrap_pyfunction!(restore_lint_interpolation, module)?)?;
    module.add_function(wrap_pyfunction!(format_lint_sql_bodies, module)?)?;
    module.add_function(wrap_pyfunction!(named_declaration_role_kind, module)?)?;
    module.add_function(wrap_pyfunction!(reject_lint_suppressions, module)?)
}
