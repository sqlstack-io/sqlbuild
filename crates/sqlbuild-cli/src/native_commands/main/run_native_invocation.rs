//! Serve one invocation without Python when its parse and project facts allow it.

use std::path::Path;

use crate::argv::models::Namespace;
use crate::native_commands::_helpers::decision::native_invocation;
use crate::native_commands::_helpers::execution::run_command;
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::NativeInvocation;

/// The exit status of an invocation `sqb` served, or why the same arguments go to Python.
pub fn run_native_invocation(
    namespace: &Namespace,
    python_cli: &Path,
    use_color: bool,
) -> Result<i32, PythonNeeded> {
    let invocation: NativeInvocation = native_invocation(namespace)?;
    run_command(&invocation, python_cli, use_color)
}
