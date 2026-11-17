//! The session this discovery pass read its declaration files with, if any.

use crate::discovery::_helpers::records::declaration_records::SESSION_MEMO_KEY;
use crate::discovery::_helpers::snapshots::native_classes::NativeDiscoverySession;
use crate::discovery::_helpers::snapshots::snapshot::snapshot_memo;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDictMethods;

/// The session this discovery pass read its declaration files with, if any.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub fn retained_discovery_session<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    Ok(snapshot_memo(project_dir)?
        .get_item(SESSION_MEMO_KEY)?
        .filter(|session| session.is_instance_of::<NativeDiscoverySession>())
        .unwrap_or_else(|| py.None().into_bound(py)))
}
