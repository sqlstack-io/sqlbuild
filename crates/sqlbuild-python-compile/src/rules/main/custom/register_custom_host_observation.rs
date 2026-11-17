//! Register the custom host observation functions and classes.

use crate::rules::_helpers::custom::custom_host_observation::CustomHostObserver;
use crate::rules::_helpers::custom::custom_host_observation::OBSERVER_ATTRIBUTE;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the custom host observation functions and classes.
pub fn register_custom_host_observation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CustomHostObserver>()?;
    module.add(OBSERVER_ATTRIBUTE, module.py().None())
}
