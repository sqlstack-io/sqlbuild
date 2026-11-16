//! Wrap a focused command projection in the established pipeline envelope.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyFloat};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Wrap a focused command projection in the established pipeline envelope.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn build_static_pipeline_result<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_static_pipeline_result",
        options,
        (&["context", "plan_output"], &[]),
    )?;
    let context: Bound<'py, PyAny> = arguments.value("context")?;
    new_record_with(
        py,
        RecordId::CompilePipelineResult,
        &[
            ("project", native_field(&context, "project")?),
            ("plan_output", arguments.value("plan_output")?),
            (
                "compile_seconds",
                native_field(&context, "compile_seconds")?,
            ),
            ("planning_seconds", PyFloat::new(py, 0.0).into_any()),
        ],
    )
}
