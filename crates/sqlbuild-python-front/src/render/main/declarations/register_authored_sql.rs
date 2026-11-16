//! Register the authored sql functions and classes.

use crate::render::_helpers::declarations::authored_sql::expand_authored_sql_with_spans;
use crate::render::_helpers::declarations::authored_sql::expand_sql_with_spans;
use crate::render::_helpers::declarations::authored_sql::substitute_sql_vars_with_spans;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the authored sql functions and classes.
pub fn register_authored_sql(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(expand_authored_sql_with_spans, module)?)?;
    module.add_function(wrap_pyfunction!(expand_sql_with_spans, module)?)?;
    module.add_function(wrap_pyfunction!(substitute_sql_vars_with_spans, module)?)?;
    Ok(())
}
