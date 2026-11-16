//! Public records of native host.

use crate::compiled_views::models::NativeCompiledModel;
use crate::planner_scope::models::ScopeData;
use pyo3::IntoPyObject;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_orchestration::declarations::models::TriggerDefinition;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use std::sync::Arc;

/// Native compiled views stay typed; caller-authored protocol implementations remain public values.
#[derive(Debug)]
pub enum ModelHandle {
    View(Py<NativeCompiledModel>),
    Public(Py<PyAny>),
}

impl ModelHandle {
    pub fn read(value: &Bound<'_, PyAny>) -> Self {
        match value.cast::<NativeCompiledModel>() {
            Ok(value) => Self::View(value.clone().unbind()),
            Err(_) => Self::Public(value.clone().unbind()),
        }
    }

    pub fn bind<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        match self {
            Self::View(value) => value.bind(py).clone().into_any(),
            Self::Public(value) => value.bind(py).clone(),
        }
    }

    pub fn clone_ref(&self, py: Python<'_>) -> Self {
        match self {
            Self::View(value) => Self::View(value.clone_ref(py)),
            Self::Public(value) => Self::Public(value.clone_ref(py)),
        }
    }
}

/// A public caller's mapping, or the compiler-owned models shared by native planning.
#[derive(Debug)]
pub enum ModelLookup {
    Public(Py<PyAny>),
    Native(Arc<ScopeData>),
}

impl ModelLookup {
    pub fn clone_ref(&self, py: Python<'_>) -> Self {
        match self {
            Self::Public(value) => Self::Public(value.clone_ref(py)),
            Self::Native(value) => Self::Native(value.clone()),
        }
    }

    pub fn get<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Public(value) => mapping_get(value.bind(py), name),
            Self::Native(value) => Ok(value
                .model(py, name)
                .unwrap_or_else(|| py.None().into_bound(py))),
        }
    }

    pub fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Public(value) => Ok(value.bind(py).clone()),
            Self::Native(value) => value.field(py, "models_by_name"),
        }
    }
}

impl<'py> From<Bound<'py, PyAny>> for ModelLookup {
    fn from(value: Bound<'py, PyAny>) -> Self {
        Self::Public(value.unbind())
    }
}

impl<'py> IntoPyObject<'py> for &ModelLookup {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        self.project(py)
    }
}

/// A job's or group's schedule on a named calendar, as its trigger before the calendar's times fill it.
#[derive(Debug)]
pub struct CalendarRequest {
    pub calendar: String,
    pub timezone: String,
    pub trigger: TriggerDefinition,
}
