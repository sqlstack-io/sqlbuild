//! Planner scope and identity contexts: built from keywords and read by planner phases.

use pyo3::prelude::{Bound, Py, PyAny, PyResult};
use pyo3::pymethods;
use pyo3::{PyClass, pyclass::boolean_struct::True};

use crate::bindings::_helpers::planner_phases::models::{
    PlannerIdentityContext, PlannerScopeResolution,
};
use crate::bindings::_helpers::planner_scope::retained::project_scope;

#[pymethods]
impl PlannerScopeResolution {
    #[new]
    #[pyo3(signature = (*, selected_scope, stale_warning_scope, inspection_scope))]
    fn new(
        selected_scope: Py<PyAny>,
        stale_warning_scope: Py<PyAny>,
        inspection_scope: Py<PyAny>,
    ) -> Self {
        Self {
            selected_scope,
            stale_warning_scope,
            inspection_scope,
        }
    }

    #[getter]
    fn selected_scope<'py>(&self, py: pyo3::Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        project_scope(self.selected_scope.bind(py))
    }

    #[getter]
    fn stale_warning_scope<'py>(&self, py: pyo3::Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        project_scope(self.stale_warning_scope.bind(py))
    }

    #[getter]
    fn inspection_scope<'py>(&self, py: pyo3::Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        project_scope(self.inspection_scope.bind(py))
    }
}

#[pymethods]
impl PlannerIdentityContext {
    #[new]
    #[pyo3(signature = (*, version_identities, stale_warning_identities))]
    fn new(version_identities: Py<PyAny>, stale_warning_identities: Py<PyAny>) -> Self {
        Self {
            version_identities,
            stale_warning_identities,
        }
    }
}

/// One field of a frozen native planner class passed as a Python object.
pub(crate) fn class_field<'py, T>(
    value: &Bound<'py, PyAny>,
    field: fn(&T) -> &Py<PyAny>,
) -> PyResult<Bound<'py, PyAny>>
where
    T: PyClass<Frozen = True> + Sync,
{
    Ok(field(value.cast::<T>()?.get()).bind(value.py()).clone())
}
