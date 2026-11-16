//! After Rules failed, write every model's compiled SQL to `target/compiled-failed/` and remove every other file there, so it holds exactly the expansion these Rules checked. A model with Rule errors starts with a header marking it not buildable and listing them. `target/compiled/` keeps the last passing compile; only an earlier manifest is removed, since it no longer describes `target/`. Returns the failing models' files, relative to `target/compiled-failed/` and sorted.

use crate::compile_command::_helpers::artifacts::target_writer;
use crate::compile_command::types::ArtifactRecorder;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// After Rules failed, write every model's compiled SQL to `target/compiled-failed/` and remove every other file there, so it holds exactly the expansion these Rules checked. A model with Rule errors starts with a header marking it not buildable and listing them. `target/compiled/` keeps the last passing compile; only an earlier manifest is removed, since it no longer describes `target/`. Returns the failing models' files, relative to `target/compiled-failed/` and sorted.
#[inline]
pub fn write_rule_failed_models<'py>(
    target_dir: &Bound<'py, PyAny>,
    (project, project_dir): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    diagnostics: &Bound<'py, PyAny>,
    recorder: ArtifactRecorder<'_>,
) -> PyResult<Vec<String>> {
    target_writer::write_rule_failed_models(
        target_dir,
        (project, project_dir),
        diagnostics,
        recorder,
    )
}
