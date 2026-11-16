//! Merged effective vars with recursive `${name}` expansion.

use crate::model_config::_helpers::config::config_templates::{
    lone_surrogate_site, resolve_effective_vars,
};
use crate::model_config::_helpers::config::template_data::applied;
use crate::model_config::_helpers::config::template_data::environment;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyString};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// Merged effective vars with recursive `${name}` expansion.
pub fn expanded_effective_vars<'py>(
    raw_values: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw_values.py();
    if let Some((name, path, byte_offset)) = lone_surrogate_site(raw_values)? {
        let error: Bound<'py, PyAny> = error_class(py, ErrorId::CompileInput)?.call(
            (format!(
                "Variable '{name}' holds a lone surrogate in {path} at UTF-8 byte \
                     {byte_offset}, which is not valid Unicode text"
            ),),
            Some(&keywords(
                py,
                &[(
                    "help",
                    PyString::new(
                        py,
                        &format!(
                            "Set '{name}' to valid Unicode text in --vars or the [vars] \
                                 table; the value is not shown because it may be a secret"
                        ),
                    )
                    .into_any(),
                )],
            )?),
        )?;
        return Err(PyErr::from_value(error));
    }
    applied(
        py,
        resolve_effective_vars(py, raw_values, environment(py)?)?,
    )
}
