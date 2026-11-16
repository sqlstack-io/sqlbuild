//! Start a session on `catalog`.

use crate::analysis::models::ProjectCatalog;
use crate::analysis_session::_helpers::session;
use crate::analysis_session::models::NativeModelAnalysisSession;
use crate::analysis_session::models::RequestRow;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyRef;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use std::path::PathBuf;

/// Start a session on `catalog`.
#[inline]
pub fn start_model_analysis_session(
    catalog: PyRef<'_, ProjectCatalog>,
    request: RequestRow,
    cache: Option<(PathBuf, String)>,
    adapter_rules: Option<(Py<PyDict>, Py<PyAny>)>,
) -> PyResult<NativeModelAnalysisSession> {
    session::start_model_analysis_session(catalog, request, cache, adapter_rules)
}
