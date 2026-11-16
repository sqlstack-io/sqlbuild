//! The coordinator of one live transient terminal line and persistent writes.

use crate::build_progress::models::TransientLineCoordinator;

use pyo3::prelude::{Bound, Py, PyAny, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::{pyfunction, wrap_pyfunction};

/// The stream a transient line is drawn on and the renderer that owns it.
pub(crate) type ActiveLine = (Py<PyAny>, Py<PyAny>);

static SHARED_COORDINATOR: PyOnceLock<Py<TransientLineCoordinator>> = PyOnceLock::new();

/// The coordinator shared by every terminal writer in this process.
pub(crate) fn shared_coordinator(py: Python<'_>) -> PyResult<Bound<'_, TransientLineCoordinator>> {
    SHARED_COORDINATOR
        .get_or_try_init(py, || Py::new(py, TransientLineCoordinator::new(py)?))
        .map(|coordinator| coordinator.bind(py).clone())
}

/// Return the coordinator shared by every terminal writer in this process.
#[pyfunction]
pub(crate) fn shared_transient_line_coordinator(
    py: Python<'_>,
) -> PyResult<Bound<'_, TransientLineCoordinator>> {
    shared_coordinator(py)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<TransientLineCoordinator>()?;
    module.add_function(wrap_pyfunction!(shared_transient_line_coordinator, module)?)
}
