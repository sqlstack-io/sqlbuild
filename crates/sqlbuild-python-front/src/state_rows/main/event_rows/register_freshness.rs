//! Register the freshness functions and classes.

use crate::state_rows::main::event_rows::age_policy::age_policy;
use crate::state_rows::main::event_rows::equivalent::equivalent;
use crate::state_rows::main::event_rows::normalize::normalize;
use crate::state_rows::main::event_rows::version_hash::version_hash;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the freshness functions and classes.
pub fn register_freshness(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version_hash, module)?)?;
    module.add_function(wrap_pyfunction!(normalize, module)?)?;
    module.add_function(wrap_pyfunction!(equivalent, module)?)?;
    module.add_function(wrap_pyfunction!(age_policy, module)?)?;
    Ok(())
}
