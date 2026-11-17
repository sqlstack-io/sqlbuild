//! Register the policy functions and classes.

use crate::event_exporting::_helpers::policy::event_export_severity_at_least;
use crate::event_exporting::_helpers::policy::lifecycle_export_policy_catalog;
use crate::event_exporting::_helpers::policy::stricter_event_export_severity;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, wrap_pyfunction};

/// Register the policy functions and classes.
pub fn register_policy(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(lifecycle_export_policy_catalog, module)?)?;
    module.add_function(wrap_pyfunction!(event_export_severity_at_least, module)?)?;
    module.add_function(wrap_pyfunction!(stricter_event_export_severity, module)?)
}
