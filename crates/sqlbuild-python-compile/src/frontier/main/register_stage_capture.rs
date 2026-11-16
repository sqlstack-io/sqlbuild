//! Register the stage capture functions and classes.

use crate::frontier::_helpers::stage_capture::mask_capture_noise;
use crate::frontier::_helpers::stage_capture::render_stage_capture;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet};
use pyo3::wrap_pyfunction;
use sqlbuild_cache::stage_capture::constants::OMITTED_ATTRIBUTES;

/// Register the stage capture functions and classes.
pub fn register_stage_capture(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py: Python<'_> = module.py();
    let omitted: Bound<'_, PyDict> = PyDict::new(py);
    for (type_name, names) in OMITTED_ATTRIBUTES {
        omitted.set_item(type_name, PyFrozenSet::new(py, names.iter())?)?;
    }
    module.add("STAGE_CAPTURE_OMITTED_ATTRIBUTES", omitted)?;
    module.add_function(wrap_pyfunction!(render_stage_capture, module)?)?;
    module.add_function(wrap_pyfunction!(mask_capture_noise, module)?)
}
