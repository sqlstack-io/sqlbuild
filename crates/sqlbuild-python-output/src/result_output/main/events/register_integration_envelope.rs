//! Register the integration envelope functions and classes.

use crate::result_output::_helpers::events::integration_envelope::IntegrationAssetResult;
use crate::result_output::_helpers::events::integration_envelope::IntegrationCheckResult;
use crate::result_output::_helpers::events::integration_envelope::IntegrationResultEnvelope;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the integration envelope functions and classes.
pub fn register_integration_envelope(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<IntegrationAssetResult>()?;
    module.add_class::<IntegrationCheckResult>()?;
    module.add_class::<IntegrationResultEnvelope>()
}
