//! Register the reuse recording functions and classes.

use crate::compile_command::main::invocation::write_reusable_compile::write_reusable_compile;
use crate::compile_command::models::CompileArtifactWrites;
use crate::compile_command::models::RecordedCompileOutput;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the reuse recording functions and classes.
pub fn register_reuse_recording(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CompileArtifactWrites>()?;
    module.add_class::<RecordedCompileOutput>()?;
    module.add_function(wrap_pyfunction!(write_reusable_compile, module)?)
}
