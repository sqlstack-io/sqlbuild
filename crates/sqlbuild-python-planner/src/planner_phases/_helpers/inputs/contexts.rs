//! Planner scope and identity contexts: built from keywords and read by planner phases.

use pyo3::prelude::{Bound, Py, PyAny, PyResult};
use pyo3::pymethods;
use pyo3::{PyClass, pyclass::boolean_struct::True};

use crate::planner_phases::models::PlannerIdentityContext;

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
