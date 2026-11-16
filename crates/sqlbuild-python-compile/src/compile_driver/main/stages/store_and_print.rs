//! Store a finished compile for reuse, then print its recorded output.

use crate::compile_command::main::invocation::write_reusable_compile::write_reusable_compile;
use crate::compile_driver::types::PhasesOutcome;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBool;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Store a finished compile for reuse, then print its recorded output.
pub fn store_and_print<'py>(
    (request, attempt): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    output: &Bound<'py, PyAny>,
    phases: PhasesOutcome<'py>,
) -> PyResult<i64> {
    let py: Python<'py> = request.py();
    let (outcome, input_reads, artifact_writes): PhasesOutcome<'py> = phases;
    let stored: PyResult<()> = write_reusable_compile(
        py,
        Some(keywords(
            py,
            &[
                ("attempt", attempt.clone()),
                ("output", output.clone()),
                ("exit_code", outcome.exit_code.into_pyobject(py)?.into_any()),
                ("input_reads", input_reads),
                ("artifact_writes", artifact_writes),
                (
                    "compile_cache_enabled",
                    PyBool::new(py, outcome.compile_cache_enabled)
                        .to_owned()
                        .into_any(),
                ),
                (
                    "artifacts_written",
                    PyBool::new(py, outcome.artifacts_written)
                        .to_owned()
                        .into_any(),
                ),
                ("dag_artifact_path", outcome.dag_artifact_path),
                ("json_output", native_field(request, "json_output")?),
            ],
        )?),
    );
    let printed: PyResult<Bound<'py, PyAny>> = output.call_method0("print_stdout");
    printed?;
    stored?;
    Ok(outcome.exit_code)
}
