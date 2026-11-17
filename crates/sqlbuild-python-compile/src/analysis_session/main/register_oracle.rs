//! Register the oracle functions and classes.

use crate::analysis_session::_helpers::oracle::_oracle_cte_fact_recovery;
use crate::analysis_session::_helpers::oracle::_oracle_names_set_operation;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the oracle functions and classes.
pub fn register_oracle(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(_oracle_cte_fact_recovery, module)?)?;
    module.add_function(wrap_pyfunction!(_oracle_names_set_operation, module)?)?;
    Ok(())
}
