//! Compile a project for consumers that construct their own focused plans.

use crate::compile_driver::main::setup::compile_project_phase::compile_project_phase;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyFloat};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Compile a project for consumers that construct their own focused plans.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn run_compile_only_pipeline<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "run_compile_only_pipeline",
        options,
        (
            &["discovered_inputs", "adapter", "options"],
            &["on_progress"],
        ),
    )?;
    let compiled: Bound<'py, PyAny> = compile_project_phase(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", arguments.value("discovered_inputs")?),
                ("adapter", arguments.value("adapter")?),
                ("options", arguments.value("options")?),
                ("on_progress", arguments.value("on_progress")?),
            ],
        )?),
    )?;
    new_record_with(
        py,
        RecordId::CompilePipelineResult,
        &[
            ("project", native_field(&compiled, "project")?),
            (
                "plan_output",
                new_record(py, RecordId::PlanOutput, (), None)?,
            ),
            (
                "compile_seconds",
                native_field(&compiled, "compile_seconds")?,
            ),
            ("planning_seconds", PyFloat::new(py, 0.0).into_any()),
        ],
    )
}
