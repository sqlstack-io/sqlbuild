//! Catalog selection uses the same named-layer resolver, with no writer layers.

use crate::contracts::_helpers::connection_layers::{connection_layers, without_kind};
use crate::contracts::_helpers::effective_config::Outcome;
use crate::contracts::_helpers::effective_config::dictionary;
use crate::contracts::_helpers::effective_config::message;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyDict, PyString};
use sqlbuild_config::effective::main::effective_connection::effective_connection;

/// Catalog selection uses the same named-layer resolver, with no writer layers.
pub fn effective_connection_fields<'py>(
    py: Python<'py>,
    configs: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    target_config: &Bound<'py, PyAny>,
    catalog: bool,
) -> PyResult<Outcome<'py, Bound<'py, PyDict>>> {
    let (layers, independent) = connection_layers(configs, target_config, catalog)?;
    Ok(match effective_connection(layers) {
        Ok(connection) => (Some(dictionary(py, without_kind(connection))?), None),
        Err(error) => {
            let message = message(py, &error)?;
            (
                None,
                Some(if independent {
                    PyString::new(py, &format!("catalog_connection: {message}"))
                } else {
                    message
                }),
            )
        }
    })
}
