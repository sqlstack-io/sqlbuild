//! What a `sqb dev` session owns: the location child's pipes and how to run `sqb` for the project.

use std::path::PathBuf;

use tokio::process::{ChildStderr, ChildStdin, ChildStdout};

/// The location child's control input, event output and log output.
#[derive(Debug)]
pub(crate) struct LocationPipes {
    pub(crate) stdin: ChildStdin,
    pub(crate) stdout: ChildStdout,
    pub(crate) stderr: ChildStderr,
}

/// The `sqb` executable, project and target the foreground connection check runs with.
#[derive(Clone, Debug)]
pub(crate) struct DevProject {
    pub(crate) executable: PathBuf,
    pub(crate) project: PathBuf,
    pub(crate) target: String,
}
