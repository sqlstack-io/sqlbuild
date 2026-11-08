//! Type recovery's revalidation on a native route, read by the dialect's own analysis.

use std::sync::Arc;

use rayon::ThreadPool;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

use crate::analysis_route::_helpers::revalidation::{revised, session_outcome, unrevised};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::{FinishedSession, ModelOutcome, PivotOutcome};
use crate::semantic_checks::models::RecoveryModel;
use crate::semantic_checks::types::RevisedBinding;

/// Each model's findings with the `poisoned` outputs untyped; an unreadable model keeps its own.
pub fn native_revalidation(
    dialect: &dyn NativeDialect,
    (finished, outcomes): (&FinishedSession, &[(ModelOutcome, PivotOutcome)]),
    models: &[&RecoveryModel],
    poisoned: &[(String, String)],
) -> Result<Vec<Vec<RevisedBinding>>, String> {
    let pool: Arc<ThreadPool> = finished.pool.clone()?;
    pool.install(|| {
        models
            .par_iter()
            .map(|model| {
                let outcome: &ModelOutcome = session_outcome(finished, outcomes, &model.name)?;
                match revised(dialect, outcome, poisoned, &finished.declared_functions) {
                    Ok(Some(rows)) => Ok(rows),
                    Ok(None) => Ok(unrevised(model)),
                    Err(reason) => Err(format!("{}: {reason}", model.name)),
                }
            })
            .collect()
    })
}
