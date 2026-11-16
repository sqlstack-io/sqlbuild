//! `build_manifest`: the dbt-style manifest bytes, whose encoding error is raised on writing.

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{PyAnyMethods, PyErr, Python};
use sqlbuild_analysis::manifest::errors::ManifestError;

use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

pub(crate) const DATETIME_MODULE: &str = "datetime";

pub(crate) fn python_error(error: ManifestError, py: Python<'_>) -> PyErr {
    match error {
        ManifestError::QueryFingerprint(message) => error_class(py, ErrorId::QueryFingerprint)
            .and_then(|class| class.call1((message,)))
            .map_or_else(|error| error, PyErr::from_value),
        ManifestError::MissingParameterType(name) => PyKeyError::new_err(name),
    }
}
