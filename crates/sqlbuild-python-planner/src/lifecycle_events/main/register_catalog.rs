//! Register the catalog functions and classes.

use crate::lifecycle_events::_helpers::catalog::lifecycle_event_catalogued;
use crate::lifecycle_events::_helpers::catalog::lifecycle_event_terminal;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::types::PyFrozenSet;
use pyo3::wrap_pyfunction;
use sqlbuild_core::lifecycle_events::constants::{
    CURRENT_DIAGNOSTIC_LOG_SCHEMA_VERSION, CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION, HOOK_PHASES,
    HOOK_TYPES, LIFECYCLE_EVENT_SCHEMA_VERSIONS, OPERATION_ADAPTERS, OPERATION_KINDS,
    OPERATION_METADATA_FIELDS, OPERATION_NAMES, OPERATION_PHASES, OPERATION_SCOPES,
    OPERATION_STRATEGIES, OPERATION_TARGET_KINDS, RESOURCE_SKIP_CODES, RESOURCE_SKIP_MODES,
};

/// Register the catalog functions and classes.
pub fn register_catalog(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    module.add(
        "CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION",
        CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION,
    )?;
    module.add(
        "CURRENT_DIAGNOSTIC_LOG_SCHEMA_VERSION",
        CURRENT_DIAGNOSTIC_LOG_SCHEMA_VERSION,
    )?;
    module.add(
        "LIFECYCLE_EVENT_SCHEMA_VERSIONS",
        PyFrozenSet::new(py, LIFECYCLE_EVENT_SCHEMA_VERSIONS)?,
    )?;
    let sets: [(&str, &[&str]); 12] = [
        ("HOOK_PHASES", &HOOK_PHASES),
        ("HOOK_TYPES", &HOOK_TYPES),
        ("OPERATION_ADAPTERS", &OPERATION_ADAPTERS),
        ("OPERATION_KINDS", &OPERATION_KINDS),
        ("OPERATION_METADATA_FIELDS", &OPERATION_METADATA_FIELDS),
        ("OPERATION_NAMES", &OPERATION_NAMES),
        ("OPERATION_PHASES", &OPERATION_PHASES),
        ("OPERATION_SCOPES", &OPERATION_SCOPES),
        ("OPERATION_STRATEGIES", &OPERATION_STRATEGIES),
        ("OPERATION_TARGET_KINDS", &OPERATION_TARGET_KINDS),
        ("RESOURCE_SKIP_CODES", &RESOURCE_SKIP_CODES),
        ("RESOURCE_SKIP_MODES", &RESOURCE_SKIP_MODES),
    ];
    for (name, values) in sets {
        module.add(name, PyFrozenSet::new(py, values)?)?;
    }
    module.add_function(wrap_pyfunction!(lifecycle_event_catalogued, module)?)?;
    module.add_function(wrap_pyfunction!(lifecycle_event_terminal, module)?)
}
