//! Model output names and lineage the semantic stages read from a finished analysis session.

use sqlbuild_analysis::assembly::analysis_session::main::finished_model_facts::finished_model_facts;
use sqlbuild_analysis::assembly::analysis_session::models::{ColumnFact, SessionModelFacts};

use sqlbuild_python_project::analysis_session::models::NativeModelAnalysisSession;

/// The facts the finished `session` kept for model `name`, which the payload left out.
pub(crate) fn session_model_facts<'a>(
    session: Option<&'a NativeModelAnalysisSession>,
    name: &str,
) -> Result<&'a SessionModelFacts, String> {
    let finished = session
        .and_then(NativeModelAnalysisSession::finished_session)
        .ok_or("a model asked for session facts without a finished session")?;
    finished_model_facts(finished, name)
        .ok_or_else(|| format!("the finished session kept no facts for model {name}"))
}

/// The output column names of a model's session facts.
pub(crate) fn column_names(facts: &SessionModelFacts) -> Option<Vec<String>> {
    let columns: &[ColumnFact] = facts.columns.as_deref()?;
    Some(columns.iter().map(|column| column.name.clone()).collect())
}
