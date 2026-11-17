//! Register the ledger functions and classes.

use crate::cost_telemetry::_helpers::ledger::read_statement_ledger;
use crate::cost_telemetry::_helpers::ledger::record_statement;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the ledger functions and classes.
pub fn register_ledger(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(record_statement, module)?)?;
    module.add_function(wrap_pyfunction!(read_statement_ledger, module)?)?;
    Ok(())
}
