//! Public records of analysis session.

use crate::project_assembly::models::DeclaredShapes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::types::PyDict;
use pyo3::types::PyFrozenSet;
use sqlbuild_model_config::header_metadata::models::DynamicFamily;
use sqlbuild_python_project::analysis_session::models::NativeModelAnalysisSession;

/// What one compile's model analysis reads: the analysed models and the relation facts.
#[derive(Debug)]
pub struct ModelAnalysisRequest<'a, 'py> {
    pub model_inputs: Vec<Bound<'py, PyAny>>,
    pub profile: Bound<'py, PyAny>,
    pub column_types: Bound<'py, PyAny>,
    pub nullability: Bound<'py, PyAny>,
    pub complete_schemas: Bound<'py, PyAny>,
    pub dynamic_families: &'a [(String, Vec<DynamicFamily>)],
    pub rich_type_inference: bool,
    /// The `AnalysisCacheContext`, or `None` when the analysis store is bypassed.
    pub cache: Bound<'py, PyAny>,
    /// Native rows of the three declaration maps, when the caller built them alongside.
    pub declared: Option<&'a DeclaredShapes>,
}

/// Each analysed model's position in the finished session, those that succeeded, and the session.
#[derive(Debug)]
pub struct ModelAnalyses<'py> {
    pub positions: Bound<'py, PyDict>,
    pub succeeded: Bound<'py, PyFrozenSet>,
    /// The finished session, or `None` when no model was analysed.
    pub session: Option<Bound<'py, NativeModelAnalysisSession>>,
}
