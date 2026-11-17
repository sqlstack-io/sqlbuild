//! Register the envelopes functions and classes.

use crate::lifecycle_events::_helpers::envelopes::diagnostic_log_check;
use crate::lifecycle_events::_helpers::envelopes::invocation_sequence_check;
use crate::lifecycle_events::_helpers::envelopes::lifecycle_event_check;
use crate::lifecycle_events::_helpers::envelopes::producer_check;
use crate::lifecycle_events::_helpers::envelopes::run_bounds_check;
use crate::lifecycle_events::_helpers::envelopes::validated_operation_hook_fields;
use crate::lifecycle_events::_helpers::envelopes::validated_operation_metadata;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the envelopes functions and classes.
pub fn register_envelopes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(lifecycle_event_check, module)?)?;
    module.add_function(wrap_pyfunction!(invocation_sequence_check, module)?)?;
    module.add_function(wrap_pyfunction!(diagnostic_log_check, module)?)?;
    module.add_function(wrap_pyfunction!(run_bounds_check, module)?)?;
    module.add_function(wrap_pyfunction!(validated_operation_metadata, module)?)?;
    module.add_function(wrap_pyfunction!(validated_operation_hook_fields, module)?)?;
    module.add_function(wrap_pyfunction!(producer_check, module)?)
}
