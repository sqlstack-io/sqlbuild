//! Start a session on `catalog`.

use crate::analysis::models::ProjectCatalog;
use crate::analysis_session::_helpers::nullability_rules::{RuleFailure, nullability_callback};
use crate::analysis_session::_helpers::session::MODEL_ANALYSIS_STORE_KIND;
use crate::analysis_session::_helpers::session::internal_error;
use crate::analysis_session::_helpers::session::session_request;
use crate::analysis_session::models::CacheStatsRow;
use crate::analysis_session::models::NativeModelAnalysisSession;
use crate::analysis_session::models::RequestRow;
use pyo3::prelude::{Py, PyAny, PyRef, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_analysis::assembly::analysis_session::main::attach_analysis_cache::attach_analysis_cache;
use sqlbuild_analysis::assembly::analysis_session::main::start_analysis_session::start_analysis_session;
use sqlbuild_analysis::assembly::analysis_session::models::{AnalysisSession, SessionRequest};
use sqlbuild_python_base::cache::main::open_store::open_store;
use sqlbuild_python_base::types::CompilerDetach;
use std::path::PathBuf;

/// Start a session on `catalog`.
pub fn start_model_analysis_session(
    catalog: PyRef<'_, ProjectCatalog>,
    request: RequestRow,
    cache: Option<(PathBuf, String)>,
    adapter_rules: Option<(Py<PyDict>, Py<PyAny>)>,
) -> PyResult<NativeModelAnalysisSession> {
    let py: Python<'_> = catalog.py();
    let mut request: SessionRequest =
        sqlbuild_python_base::compile_context::main::delta::inject_relations::inject_relations(
            session_request(request),
        )?;
    let rule_failure: RuleFailure = RuleFailure::default();
    let dialect: String = request.dialect.clone();
    request.nullability_callback = adapter_rules
        .map(|(rules, nullability)| nullability_callback(rules, nullability, rule_failure.clone()));
    let catalog = &catalog.inner;
    let mut session: AnalysisSession = py
        .compiler_detach(|| start_analysis_session(request, catalog))
        .map_err(|reason| internal_error(&reason))?;
    let cache: Option<(PathBuf, String)> = cache.filter(|_| {
        sqlbuild_python_base::compile_context::main::delta::injection::injection().is_none()
    });
    let mut cache_path: Option<PathBuf> = None;
    let mut cache_stats: Option<CacheStatsRow> = None;
    if let Some((path, environment)) = cache {
        match open_store(py, &path, MODEL_ANALYSIS_STORE_KIND, &environment) {
            Ok(store) => {
                attach_analysis_cache(&mut session, store);
                cache_path = Some(path);
            }
            Err(error) => {
                cache_stats = Some(CacheStatsRow {
                    hits: 0,
                    misses: 0,
                    stored: 0,
                    failure: Some(error.to_string()),
                });
            }
        }
    }
    Ok(NativeModelAnalysisSession {
        inner: Some(session),
        finished: None,
        outcomes: Vec::new(),
        sharing: (0, 0),
        cache_path,
        cache_stats,
        rule_failure,
        dialect,
    })
}
