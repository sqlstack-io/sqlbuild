//! Report and skip a failing file when the caller tolerates file faults.

use pyo3::exceptions::{PyOSError, PySyntaxError, PyUnicodeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyTuple;

use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

const PROJECT_DIR_MARKER: &str = ".";

/// The files one discovery builds, with the caller's optional `on_fault` reporter.
pub(crate) struct FileFaults<'py> {
    pub(crate) project_dir: Bound<'py, PyAny>,
    pub(crate) on_fault: Option<Bound<'py, PyAny>>,
}

impl<'py> FileFaults<'py> {
    /// Whether `on_fault` takes this error instead of the caller.
    fn tolerated(&self, error: &PyErr) -> bool {
        let py: Python<'py> = self.project_dir.py();
        self.on_fault.is_some()
            && (error.is_instance_of::<PyOSError>(py)
                || error.is_instance_of::<PyUnicodeError>(py)
                || error.is_instance_of::<PyValueError>(py)
                || error.is_instance_of::<PySyntaxError>(py))
    }

    /// The built record; a tolerated failure for `relative_path` is reported and skipped.
    pub(crate) fn kept(
        &self,
        relative_path: &Bound<'py, PyAny>,
        built: PyResult<Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let error: PyErr = match built {
            Ok(record) => return Ok(Some(record)),
            Err(error) => error,
        };
        let Some(on_fault) = self.on_fault.as_ref().filter(|_| self.tolerated(&error)) else {
            return Err(error);
        };
        let py: Python<'py> = self.project_dir.py();
        let message: Bound<'py, PyAny> = error
            .value(py)
            .str()?
            .call_method1("replace", (self.project_dir.str()?, PROJECT_DIR_MARKER))?;
        on_fault.call1((new_record_with(
            py,
            RecordId::DiscoveryFileFault,
            &[("path", relative_path.clone()), ("message", message)],
        )?,))?;
        Ok(None)
    }
}

/// The built records as the tuple a discoverer returns.
pub(crate) fn record_tuple<'py>(
    py: Python<'py>,
    records: Vec<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, records)
}
