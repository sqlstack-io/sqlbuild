//! Register the custom evaluation functions and classes.

use crate::rules::_helpers::custom::custom_evaluation::custom_rule_fact_digest;
use crate::rules::_helpers::custom::custom_host_columns::custom_host_declared_columns;
use crate::rules::_helpers::custom::custom_host_payload::CUSTOM_HOST_PAYLOAD_MAGIC;
use crate::rules::_helpers::custom::custom_host_payload::custom_host_payload;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::types::PyBytes;
use pyo3::wrap_pyfunction;

/// Register the custom evaluation functions and classes.
pub fn register_custom_evaluation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(custom_host_payload, module)?)?;
    module.add_function(wrap_pyfunction!(custom_host_declared_columns, module)?)?;
    module.add(
        "CUSTOM_HOST_PAYLOAD_MAGIC",
        PyBytes::new(module.py(), CUSTOM_HOST_PAYLOAD_MAGIC),
    )?;
    module.add_function(wrap_pyfunction!(custom_rule_fact_digest, module)?)
}
