//! Public model-map inputs and projections around the native planner's model lookup.

use std::sync::Arc;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};

use crate::bindings::_helpers::compiled_views::compiled_model_view::NativeCompiledModel;
use crate::bindings::_helpers::planner_scope::retained::ScopeData;
use crate::bindings::_helpers::records::fields::mapping_get;

/// Native compiled views stay typed; caller-authored protocol implementations remain public values.
#[derive(Debug)]
pub(crate) enum ModelHandle {
    View(Py<NativeCompiledModel>),
    Public(Py<PyAny>),
}

impl ModelHandle {
    pub(crate) fn read(value: &Bound<'_, PyAny>) -> Self {
        match value.cast::<NativeCompiledModel>() {
            Ok(value) => Self::View(value.clone().unbind()),
            Err(_) => Self::Public(value.clone().unbind()),
        }
    }

    pub(crate) fn bind<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        match self {
            Self::View(value) => value.bind(py).clone().into_any(),
            Self::Public(value) => value.bind(py).clone(),
        }
    }

    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
        match self {
            Self::View(value) => Self::View(value.clone_ref(py)),
            Self::Public(value) => Self::Public(value.clone_ref(py)),
        }
    }
}

/// A public caller's mapping, or the compiler-owned models shared by native planning.
#[derive(Debug)]
pub(crate) enum ModelLookup {
    Public(Py<PyAny>),
    Native(Arc<ScopeData>),
}

impl ModelLookup {
    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
        match self {
            Self::Public(value) => Self::Public(value.clone_ref(py)),
            Self::Native(value) => Self::Native(value.clone()),
        }
    }

    pub(crate) fn get<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Public(value) => mapping_get(value.bind(py), name),
            Self::Native(value) => Ok(value
                .model(py, name)
                .unwrap_or_else(|| py.None().into_bound(py))),
        }
    }

    pub(crate) fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
