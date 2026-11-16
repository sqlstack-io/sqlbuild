//! Compile both environments and resolve selected diffable model names.

use crate::compile_driver::_helpers::stages::diff::diff_model_names;
use crate::compile_driver::main::setup::build_compiled_project::build_compiled_project;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Compile both environments and resolve selected diffable model names.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn run_diff_pipeline<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "run_diff_pipeline",
        options,
        (
            &[
                "discovered_inputs",
                "adapter",
                "from_target",
                "to_target",
                "resolved_connection",
                "no_sql_validation",
                "select",
                "exclude",
            ],
            &["cli_vars", "external_sql_reference_resolver"],
        ),
    )?;
    let compile = |target: &str| -> PyResult<Bound<'py, PyAny>> {
        build_compiled_project(
            py,
            Some(keywords(
                py,
                &[
                    ("discovered_inputs", arguments.value("discovered_inputs")?),
                    ("adapter", arguments.value("adapter")?),
                    ("selected_target", arguments.value(target)?),
                    (
                        "resolved_connection",
                        arguments.value("resolved_connection")?,
                    ),
                    ("no_sql_validation", arguments.value("no_sql_validation")?),
                    ("cli_vars", arguments.value("cli_vars")?),
                    (
                        "external_sql_reference_resolver",
                        arguments.value("external_sql_reference_resolver")?,
                    ),
                ],
            )?),
        )
    };
    let left: Bound<'py, PyAny> = compile("from_target")?;
    let right: Bound<'py, PyAny> = compile("to_target")?;
    let names: Bound<'py, PyTuple> = diff_model_names(
        &right,
        &arguments.value("select")?,
        &arguments.value("exclude")?,
    )?;
    PyTuple::new(py, [left, right, names.into_any()])
}
