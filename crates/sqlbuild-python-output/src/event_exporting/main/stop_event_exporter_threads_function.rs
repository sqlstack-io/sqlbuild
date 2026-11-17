//! `stop_event_exporter_threads` as a function of `module`.

use crate::event_exporting::_helpers::dispatcher::stop_event_exporter_threads;

/// `stop_event_exporter_threads` as a function of `module`.
pub fn stop_event_exporter_threads_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(stop_event_exporter_threads, module)
}
