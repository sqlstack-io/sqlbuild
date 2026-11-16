//! Register the rule cache functions and classes.

use crate::rules::_helpers::rule::rule_cache::encode_sql_rule_findings;
use crate::rules::_helpers::rule::rule_cache::load_sql_rule_cache;
use crate::rules::_helpers::rule::rule_cache::sql_file_rule_key;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the rule cache functions and classes.
pub fn register_rule_cache(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(sql_file_rule_key, module)?)?;
    module.add_function(wrap_pyfunction!(load_sql_rule_cache, module)?)?;
    module.add_function(wrap_pyfunction!(encode_sql_rule_findings, module)?)?;
    Ok(())
}
