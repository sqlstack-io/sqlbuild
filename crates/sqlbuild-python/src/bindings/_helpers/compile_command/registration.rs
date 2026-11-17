//! Register the `sqb compile` report, artifact writing and reuse recording.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

use crate::bindings::_helpers::compile_command::models::CompileReuseAttempt;

use crate::bindings::_helpers::compile_command::prepared_artifacts;
use crate::bindings::_helpers::compile_command::report;
use crate::bindings::_helpers::compile_command::reuse_attempt;
use crate::bindings::_helpers::compile_command::reuse_recording;
use crate::bindings::_helpers::compile_command::runtime_identity;
use crate::bindings::_helpers::compile_command::startup_inputs;
use crate::bindings::_helpers::compile_command::target_writer;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CompileReuseAttempt>()?;
    report::register(module)?;
    reuse_attempt::register(module)?;
    reuse_recording::register(module)?;
    runtime_identity::register(module)?;
    startup_inputs::register(module)?;
    target_writer::register(module)?;
    prepared_artifacts::register(module)
}
