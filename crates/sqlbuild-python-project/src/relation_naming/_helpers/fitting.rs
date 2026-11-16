//! Generated relation names fitted within identifier limits, raising `SharedInputError`.

use pyo3::prelude::{Bound, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::identifier_fitting::main::fit_artifact_logical_name::fit_artifact_logical_name as fit_logical;
use sqlbuild_analysis::identifier_fitting::main::fit_auxiliary_relation_name::fit_auxiliary_relation_name as fit_auxiliary;
use sqlbuild_host::entries::models::FacadeHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;

use sqlbuild_python_base::boundary::models::Request;

pub(crate) fn shared_error(py: Python<'_>, message: String) -> PyErr {
    host_error(py, FacadeHost::SharedInputError, &message)
}

/// A readable logical component fitted after a fixed prefix, with a hash suffix when shortened.
#[pyfunction]
#[pyo3(signature = (**options))]
fn fit_artifact_logical_name(options: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let request = Request::new("fit_artifact_logical_name", options)?;
    fitted_logical_name(
        request.py(),
        (
            &request.get("logical_name")?.extract::<String>()?,
            &request.get("fixed_prefix")?.extract::<String>()?,
        ),
        request.get("identifier_limit")?.extract()?,
        &request.get("artifact_label")?.extract::<String>()?,
    )
}

/// `fit_artifact_logical_name`, raising `SharedInputError` when the name cannot fit.
pub(crate) fn fitted_logical_name(
    py: Python<'_>,
    names: (&str, &str),
    identifier_limit: i64,
    artifact_label: &str,
) -> PyResult<String> {
    fit_logical(names, identifier_limit, artifact_label)
        .map_err(|message| shared_error(py, message))
}

/// `base_name + suffix` fitted within the limit, keeping the suffix and a base hash.
#[pyfunction]
#[pyo3(signature = (*, base_name, suffix, identifier_limit))]
fn fit_auxiliary_relation_name(
    py: Python<'_>,
    base_name: &str,
    suffix: &str,
    identifier_limit: i64,
) -> PyResult<String> {
    fit_auxiliary(base_name, suffix, identifier_limit).map_err(|message| shared_error(py, message))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(fit_artifact_logical_name, module)?)?;
    module.add_function(wrap_pyfunction!(fit_auxiliary_relation_name, module)?)?;
    Ok(())
}
