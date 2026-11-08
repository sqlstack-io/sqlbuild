//! Type recovery's revalidation on a native route, read by the dialect's own analysis.

use std::sync::Arc;

use rayon::ThreadPool;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

use crate::analysis_route::_helpers::revalidation::{revised, session_outcome};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::{FinishedSession, ModelOutcome, PivotOutcome};
use crate::semantic_checks::types::RevisedBinding;

/// Each named model's findings, at the session's positions, with the `poisoned` outputs untyped.
pub fn native_revalidation(
    dialect: &dyn NativeDialect,
    (finished, outcomes): (&FinishedSession, &[(ModelOutcome, PivotOutcome)]),
    models: &[String],
    poisoned: &[(String, String)],
) -> Result<Vec<Vec<RevisedBinding>>, String> {
    let pool: Arc<ThreadPool> = finished.pool.clone()?;
    pool.install(|| {
        models
            .par_iter()
            .map(|name| {
                let outcome: &ModelOutcome = session_outcome(finished, outcomes, name)?;
                revised(dialect, outcome, poisoned, &finished.declared_functions)
                    .map_err(|reason| format!("{name}: {reason}"))
            })
            .collect()
    })
}
