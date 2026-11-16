//! Public traits and type aliases of model config.

use crate::model_config::main::config::config_node::config_node;
use crate::model_config::models::HostHandle;
use crate::model_config::models::TypedConfigValues;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_model_config::types::AuthoredNode;
use std::collections::HashMap;
use std::sync::Arc;

/// A config value converted from Python.
pub type PyConfigNode = ConfigNode<HostHandle>;

/// A parsed leaf projected only when a public contract needs its Python value.
pub trait HeaderValue: AuthoredNode {
    fn object<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>>;

    fn retained(&self, py: Python<'_>) -> PyResult<PyConfigNode> {
        config_node(&self.object(py)?)
    }

    fn exact_text(&self, py: Python<'_>) -> PyResult<String> {
        self.object(py)?.extract()
    }
}

/// A built config's string-keyed tree; values shared across reused configs stay native.
pub type BuiltConfigValues = Arc<HashMap<String, Arc<PyConfigNode>>>;

/// A config record's values and the typed nodes read of them, each key converted on first read.
pub type ConfigValues<'py> = (Bound<'py, PyAny>, Bound<'py, TypedConfigValues>);
