//! Public traits and type aliases of compile command.

use crate::compile_command::models::CompileArtifactWrites;
use pyo3::prelude::Py;

/// Where a compile notes the artifacts it writes; `None` outside a recorded compile.
pub type ArtifactRecorder<'a> = Option<&'a Py<CompileArtifactWrites>>;
