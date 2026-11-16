//! Discover the (selected) model files natively, reporting failing files to `on_fault`.

use crate::discovery::main::records::discovered_model_files::discovered_model_files;
use crate::discovery::models::ModelFileSelection;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Discover the (selected) model files natively, reporting failing files to `on_fault`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn discover_model_files<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "discover_model_files",
        options,
        (
            &[
                "project_dir",
                "extract_implicit_alias_columns",
                "extract_output_column_locations",
            ],
            &["selected_model_names", "on_fault"],
        ),
    )?;
    discovered_model_files(
        &arguments.value("project_dir")?,
        (
            arguments
                .value("extract_implicit_alias_columns")?
                .extract()?,
            arguments
                .value("extract_output_column_locations")?
                .extract()?,
        ),
        ModelFileSelection {
            selected_model_names: Some(arguments.value("selected_model_names")?),
            on_fault: Some(arguments.value("on_fault")?),
        },
    )
}
