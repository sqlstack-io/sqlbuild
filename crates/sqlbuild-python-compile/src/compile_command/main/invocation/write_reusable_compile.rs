//! Store a reusable full compile, or drop the stored one when it cannot be reused.

use crate::compile_command::_helpers::invocation::reuse_attempt::write_launch_record;
use crate::compile_command::_helpers::invocation::reuse_recording::MISS_OUTCOME;
use crate::compile_command::_helpers::invocation::reuse_recording::record;
use crate::compile_command::_helpers::invocation::reuse_recording::reuse_logger;
use crate::compile_command::models::CompileFacts;
use pyo3::exceptions::{PyException, PyOSError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Store a reusable full compile, or drop the stored one when it cannot be reused.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn write_reusable_compile(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<()> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "write_reusable_compile",
        options,
        (
            &[
                "attempt",
                "output",
                "exit_code",
                "input_reads",
                "artifact_writes",
                "compile_cache_enabled",
                "artifacts_written",
                "dag_artifact_path",
                "json_output",
            ],
            &[],
        ),
    )?;
    let attempt: Bound<'_, PyAny> = arguments.value("attempt")?;
    let entry_path: Bound<'_, PyAny> = native_field(&attempt, "entry_path")?;
    let native: Bound<'_, PyAny> = native_field(&attempt, "native")?;
    if !native_field(&attempt, "outcome")?.eq(MISS_OUTCOME)?
        || entry_path.is_none()
        || native.is_none()
    {
        return Ok(());
    }
    let facts: CompileFacts<'_> = CompileFacts {
        native,
        output: arguments.value("output")?,
        exit_code: arguments.value("exit_code")?,
        input_reads: arguments.value("input_reads")?,
        artifact_writes: arguments.value("artifact_writes")?,
        compile_cache_enabled: arguments.flag("compile_cache_enabled")?,
        artifacts_written: arguments.flag("artifacts_written")?,
        dag_artifact_path: arguments.value("dag_artifact_path")?,
        json_output: arguments.value("json_output")?,
    };
    let stored: bool = match record(&facts) {
        Ok(stored) => stored,
        Err(error) if error.is_instance_of::<PyException>(py) => {
            let _ = reuse_logger(py)?.call_method(
                "debug",
                ("Compile reuse did not store this compile",),
                Some(&keywords(
                    py,
                    &[("exc_info", error.value(py).clone().into_any())],
                )?),
            )?;
            let unlinked: PyResult<Bound<'_, PyAny>> = entry_path.call_method(
                "unlink",
                (),
                Some(&keywords(
                    py,
                    &[("missing_ok", true.into_bound_py_any(py)?)],
                )?),
            );
            if let Err(error) = unlinked
                && !error.is_instance_of::<PyOSError>(py)
            {
                return Err(error);
            }
            false
        }
        Err(error) => return Err(error),
    };
    if stored {
        write_launch_record(&attempt)?;
    }
    Ok(())
}
