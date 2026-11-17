//! Note the full compile's report, exit code and artifact writes, when a check is armed.

use crate::compile_command::models::CompileArtifactWrites;
use crate::compile_command::models::RecordedCompileOutput;
use crate::compile_delta::_helpers::verify::ACTUAL;
use crate::compile_delta::_helpers::verify::Actual;
use crate::compile_delta::_helpers::verify::EXPECTED;
use crate::compile_delta::_helpers::verify::locked;
use pyo3::prelude::{Bound, PyAny, PyResult};
use std::collections::HashMap;

/// Note the full compile's report, exit code and artifact writes, when a check is armed.
pub fn full_compile_finished(
    output: &Bound<'_, PyAny>,
    (exit_code, artifacts_written): (i64, bool),
    artifact_writes: &Bound<'_, PyAny>,
) -> PyResult<()> {
    if locked(&EXPECTED).is_none() {
        return Ok(());
    }
    let (_, stdout): (Vec<String>, Option<String>) = output
        .cast::<RecordedCompileOutput>()?
        .get()
        .texts(output.py())?;
    let written: HashMap<String, Option<String>> = artifact_writes
        .cast::<CompileArtifactWrites>()
        .map(|writes| writes.get().written_digests())
        .unwrap_or_default();
    *locked(&ACTUAL) = Some(Actual {
        stdout,
        exit_code: Some(exit_code),
        artifacts_written,
        written,
    });
    Ok(())
}
