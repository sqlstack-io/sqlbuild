//! Add the capture classes and run `stop_output_capture_threads` at interpreter exit.

use crate::output_capture::_helpers::dispatcher::OutputCaptureDispatcher;
use crate::output_capture::_helpers::dispatcher::OutputDeliveryWorker;
use crate::output_capture::_helpers::dispatcher::stop_output_capture_threads;
use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyModuleMethods, PyResult, wrap_pyfunction};
use sqlbuild_python_base::boundary::main::imported::imported;

/// Add the capture classes and run `stop_output_capture_threads` at interpreter exit.
pub fn register_dispatcher(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let hook = wrap_pyfunction!(stop_output_capture_threads, module)?;
    module.add_function(hook.clone())?;
    imported(module.py(), "atexit", "register")?.call1((hook,))?;
    module.add_class::<OutputCaptureDispatcher>()?;
    module.add_class::<OutputDeliveryWorker>()
}
