//! The public authoring records enter native compilation here, never scheduler execution.

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, Python};

pub(crate) fn validate_jobs(
    py: Python<'_>,
    declarations: &[Bound<'_, PyAny>],
) -> Result<(), PyErr> {
    let job = py.import("sqlbuild.orchestration")?.getattr("Job")?;
    for declaration in declarations {
        if !declaration.is_instance(&job)? {
            return Err(PyTypeError::new_err(
                "snapshot declarations must be sqlbuild.orchestration.Job objects",
            ));
        }
    }
    Ok(())
}
