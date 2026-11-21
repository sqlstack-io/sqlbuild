//! Public records of orchestration reads.

use crate::orchestration_reads::_helpers::orchestration_declarations::authored_setting;
use pyo3::FromPyObject;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;
use serde_json::Value;
use sqlbuild_orchestration::declarations::models::OnFailure;
use sqlbuild_orchestration::declarations::models::Overlap;
use std::collections::BTreeMap;

#[derive(Debug, FromPyObject)]
pub struct AuthoredSensorCondition {
    pub kind: String,
    #[pyo3(default)]
    pub kinds: Vec<String>,
    #[pyo3(default)]
    pub job: Option<String>,
    #[pyo3(default)]
    pub executed_successfully: Option<Vec<String>>,
    #[pyo3(default)]
    pub select: Option<String>,
    #[pyo3(default, from_py_with = authored_setting)]
    pub after_seconds: Value,
}

/// A `@sensor(...)` declaration; `name` and `function` are `None` until it decorates a function.
#[derive(Debug, FromPyObject)]
pub struct AuthoredSensor {
    pub name: Option<String>,
    pub description: String,
    pub when: Option<AuthoredSensorCondition>,
    pub function: Option<pyo3::Py<PyAny>>,
    #[pyo3(from_py_with = authored_setting)]
    pub minimum_interval_seconds: Value,
    #[pyo3(from_py_with = authored_setting)]
    pub timeout_seconds: Value,
    pub default_status: String,
    pub targets: Vec<String>,
    #[pyo3(attribute("_declared_at"))]
    pub declared_at: Option<(String, u32)>,
}

/// A job's or group's owner, overlap and failure policy, as publication records them.
#[derive(Debug)]
pub struct RunControls {
    pub owner: Option<String>,
    pub overlap: Overlap,
    pub on_failure: Option<OnFailure>,
}

/// The calendars, by name with their descriptions, and the sensors that importing the modules defines.
#[derive(Debug)]
pub struct ImportedDeclarations<'py> {
    pub calendars: BTreeMap<String, Bound<'py, PyAny>>,
    pub calendar_descriptions: BTreeMap<String, String>,
    pub sensors: Vec<AuthoredSensor>,
}

/// How a provider class instantiates from the environment.
#[derive(Debug)]
pub enum ProviderState<'py> {
    Ready(Bound<'py, PyAny>),
    /// Only required secret settings are unset: their environment variables, sorted.
    MissingSecrets(Vec<String>),
    /// Another setting is invalid: pydantic's validation error.
    Invalid(PyErr),
}
