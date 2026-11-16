//! Register the `sqb compile` report, artifact writing and reuse recording.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

use sqlbuild_python_compile::compile_command::models::CompileReuseAttempt;

use sqlbuild_python_compile::compile_command::main::artifacts::register_prepared_artifacts::register_prepared_artifacts;
use sqlbuild_python_compile::compile_command::main::artifacts::register_target_writer::register_target_writer;
use sqlbuild_python_compile::compile_command::main::invocation::register_report::register_report;
use sqlbuild_python_compile::compile_command::main::invocation::register_reuse_attempt::register_reuse_attempt;
use sqlbuild_python_compile::compile_command::main::invocation::register_reuse_recording::register_reuse_recording;
use sqlbuild_python_compile::compile_command::main::invocation::register_runtime_identity::register_runtime_identity;
use sqlbuild_python_compile::compile_command::main::invocation::register_startup_inputs::register_startup_inputs;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CompileReuseAttempt>()?;
    register_report(module)?;
    register_reuse_attempt(module)?;
    register_reuse_recording(module)?;
    register_runtime_identity(module)?;
    register_startup_inputs(module)?;
    register_target_writer(module)?;
    register_prepared_artifacts(module)
}
