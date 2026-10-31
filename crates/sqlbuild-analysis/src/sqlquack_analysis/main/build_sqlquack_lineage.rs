//! `build_sqlquack_lineage`: rich column lineage from SQLQuack, for Snowflake and DuckDB.

use std::collections::HashMap;

use rayon::ThreadPool;
use sqlquack_semantics::lineage::models::SchemaColumn;

use crate::lineage::models::{RichLineageOutcome, RichLineageRequest};
use crate::sqlquack_analysis::_helpers::rich::{
    known_columns, learning_pass, lineage_pool, model_keys,
};
use crate::sqlquack_analysis::_helpers::schema::sqlquack_dialect;
use crate::sqlquack_analysis::constants::MAX_LEARNING_PASSES;
use crate::sqlquack_analysis::models::LearningPass;

/// One outcome per model, in request order; a model's own complete outputs are learnt for readers.
pub fn build_sqlquack_lineage(
    request: &RichLineageRequest,
    names: &[String],
) -> Result<Vec<RichLineageOutcome>, String> {
    let Some((dialect, profile)) = sqlquack_dialect(&request.dialect) else {
        return Err(format!(
            "SQLQuack does not analyse the {} dialect",
            request.dialect
        ));
    };
    let mut known: HashMap<String, Vec<SchemaColumn>> = known_columns(request, profile);
    let keys: Vec<String> = model_keys(names, profile);
    let pool: ThreadPool = lineage_pool(request.models.len())?;
    let mut outcomes: Vec<Option<RichLineageOutcome>> = vec![None; request.models.len()];
    let mut pending: Vec<usize> = (0..request.models.len()).collect();
    for _ in 0..MAX_LEARNING_PASSES {
        let pass: LearningPass = learning_pass(
            (request, &keys),
            (&known, &pending),
            (dialect, profile),
            &pool,
        )?;
        known.extend(pass.learnt);
        for (index, (outcome, _)) in pass.traced {
            outcomes[index] = Some(outcome);
        }
        pending = pass.readers;
        if pending.is_empty() {
            break;
        }
    }
    Ok(outcomes.into_iter().flatten().collect())
}
