//! Each model's analysis by name and the finished session; a native internal failure raises.

use crate::analysis_session::_helpers::model_analyses::analysis_store;
use crate::analysis_session::_helpers::model_analyses::run_session;
use crate::analysis_session::_helpers::model_analyses::session_request;
use crate::analysis_session::models::ModelAnalyses;
use crate::analysis_session::models::ModelAnalysisRequest;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::analysis_session::main::nullability_rules::nullability_rules;
use sqlbuild_python_project::analysis_session::main::start_model_analysis_session::start_model_analysis_session;
use sqlbuild_python_project::analysis_session::models::{NativeModelAnalysisSession, RequestRow};

/// Each model's analysis by name and the finished session; a native internal failure raises.
pub fn analyze_models<'py>(
    request: &ModelAnalysisRequest<'_, 'py>,
) -> PyResult<ModelAnalyses<'py>> {
    let py: Python<'py> = request.profile.py();
    if request.model_inputs.is_empty() {
        return Ok(ModelAnalyses {
            positions: PyDict::new(py),
            succeeded: PyFrozenSet::empty(py)?,
            session: None,
        });
    }
    let catalog: Bound<'py, PyAny> = native_field(&request.profile, "binding_catalog")?;
    let (rules, adapter_rules) = nullability_rules(&request.profile)?;
    let session_request: RequestRow = session_request(request, &catalog, rules)?;
    let (cache, adapter) = match adapter_rules {
        Some((adapter, nullability)) => (None, Some((adapter, nullability))),
        None => (analysis_store(request)?, None),
    };
    let session: Bound<'py, NativeModelAnalysisSession> = Bound::new(
        py,
        start_model_analysis_session(
            native_field(&catalog, "native")?.extract()?,
            session_request,
            cache,
            adapter,
        )?,
    )?;
    run_session(request, &catalog, &session)
}
