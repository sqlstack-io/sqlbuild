//! Register the normalization functions and classes.

use crate::analysis::_helpers::normalization::binding_diagnostics;
use crate::analysis::_helpers::normalization::normalize_analysis_sqls;
use crate::analysis::_helpers::normalization::normalize_dialect_sql;
use crate::analysis::main::normalize_analysis_sql::normalize_analysis_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the normalization functions and classes.
pub fn register_normalization(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(normalize_analysis_sql, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_dialect_sql, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_analysis_sqls, module)?)?;
    module.add_function(wrap_pyfunction!(binding_diagnostics, module)?)?;
    Ok(())
}
