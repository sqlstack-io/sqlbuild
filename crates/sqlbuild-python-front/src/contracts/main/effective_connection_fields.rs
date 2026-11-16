//! Catalog selection uses the same named-layer resolver, with no writer layers.

use crate::contracts::_helpers::effective_config::Key;
use crate::contracts::_helpers::effective_config::Outcome;
use crate::contracts::_helpers::effective_config::Pairs;
use crate::contracts::_helpers::effective_config::Value;
use crate::contracts::_helpers::effective_config::dictionary;
use crate::contracts::_helpers::effective_config::message;
use crate::contracts::_helpers::effective_config::named;
use crate::contracts::_helpers::effective_config::optional_key;
use crate::contracts::_helpers::effective_config::pairs;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyString};
use sqlbuild_config::effective::main::effective_connection::effective_connection;
use sqlbuild_config::effective::models::ConnectionLayers;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Catalog selection uses the same named-layer resolver, with no writer layers.
pub fn effective_connection_fields<'py>(
    py: Python<'py>,
    (project_config, local_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    target_config: &Bound<'py, PyAny>,
    catalog: bool,
) -> PyResult<Outcome<'py, Bound<'py, PyDict>>> {
    let catalog_name = if catalog && !target_config.is_none() {
        optional_key(&native_field(target_config, "catalog_connection")?)?
    } else {
        None
    };
    let independent = catalog_name.is_some();
    let target: Option<(Option<Key<'py>>, Pairs<'py>)> = if independent {
        Some((catalog_name, Vec::new()))
    } else if target_config.is_none() {
        None
    } else {
        Some((
            optional_key(&native_field(target_config, "connection_name")?)?,
            pairs(&native_field(target_config, "connection")?)?,
        ))
    };
    let layers: ConnectionLayers<Key<'py>, Value<'py>> = ConnectionLayers {
        project: if independent {
            Vec::new()
        } else {
            pairs(&native_field(project_config, "connection")?)?
        },
        target,
        project_named: named(&native_field(project_config, "connections")?)?,
        local_named: named(&native_field(local_config, "connections")?)?,
        local: if independent {
            Vec::new()
        } else {
            pairs(&native_field(local_config, "connection")?)?
        },
    };
    Ok(match effective_connection(layers) {
        Ok(connection) => (Some(dictionary(py, connection)?), None),
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
