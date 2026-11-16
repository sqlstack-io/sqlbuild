//! Register the observation functions and classes.

use crate::freshness::_helpers::observation::observe_adapter_sources_freshness;
use crate::freshness::_helpers::observation::observe_configured_source_freshness;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the observation functions and classes.
pub fn register_observation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        observe_configured_source_freshness,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(observe_adapter_sources_freshness, module)?)?;
    Ok(())
}
