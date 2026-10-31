//! Rich column lineage from SQLQuack's syntax tree and lineage, for Snowflake and DuckDB.

use std::collections::HashMap;

use rayon::ThreadPool;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{LineageContext, SchemaColumn};
use sqlquack_syntax::Dialect;

use crate::lineage::_helpers::references::physical_resource_name;
use crate::lineage::_helpers::rich_lineage::schema_names;
use crate::lineage::constants::{RICH_LINEAGE_WORKER_STACK_BYTES, RICH_LINEAGE_WORKERS};
use crate::lineage::models::{LineageResourceType, RichLineageOutcome, RichLineageRequest};
use crate::sqlquack_analysis::_helpers::rich::{model_lineage, reads_learnt};
use crate::sqlquack_analysis::_helpers::schema::{
    RelationColumns, schema_columns, sqlquack_dialect,
};
use crate::sqlquack_analysis::constants::MAX_LEARNING_PASSES;
use crate::sqlquack_analysis::types::TracedModel;

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
    let model_keys: Vec<String> = model_keys(names, profile);
    let pool: ThreadPool = rayon::ThreadPoolBuilder::new()
        .num_threads(RICH_LINEAGE_WORKERS.min(request.models.len().max(1)))
        .stack_size(RICH_LINEAGE_WORKER_STACK_BYTES)
        .build()
        .map_err(|error| error.to_string())?;
    let mut outcomes: Vec<Option<RichLineageOutcome>> = vec![None; request.models.len()];
    let mut pending: Vec<usize> = (0..request.models.len()).collect();
    for _ in 0..MAX_LEARNING_PASSES {
        let schema = RelationColumns(known.clone());
        let context = LineageContext {
            profile,
            schema: &schema,
            sql: None,
        };
        let traced = trace_pass(request, &pending, (dialect, context), &pool)?;
        let mut learnt: Vec<String> = Vec::new();
        for (index, (outcome, outputs)) in traced {
            let key: &String = &model_keys[index];
            let complete: bool = matches!(
                outcome,
                RichLineageOutcome::Built {
                    has_star: false,
                    ..
                }
            );
            if complete && !outputs.is_empty() && known.get(key) != Some(&outputs) {
                let _ = known.insert(key.clone(), outputs);
                learnt.push(key.clone());
            }
            outcomes[index] = Some(outcome);
        }
        pending = (0..request.models.len())
            .filter(|index| reads_learnt(&request.models[*index], &learnt, context))
            .collect();
        if pending.is_empty() {
            break;
        }
    }
    Ok(outcomes.into_iter().flatten().collect())
}

/// The request schema's columns by resolved physical name, for resources that list any.
fn known_columns(
    request: &RichLineageRequest,
    profile: &dyn DialectProfile,
) -> HashMap<String, Vec<SchemaColumn>> {
    request
        .schema
        .iter()
        .map(schema_names)
        .filter(|resource| !resource.columns.is_empty())
        .map(|resource| {
            let physical: String = physical_resource_name(resource.resource_type, &resource.name);
            let columns: Vec<SchemaColumn> =
                schema_columns(resource.columns.iter().map(String::as_str));
            (profile.resolved_identifier(&physical, false), columns)
        })
        .collect()
}

/// Each named model's resolved physical name.
fn model_keys(names: &[String], profile: &dyn DialectProfile) -> Vec<String> {
    names
        .iter()
        .map(|name| {
            profile.resolved_identifier(
                &physical_resource_name(LineageResourceType::Model, name),
                false,
            )
        })
        .collect()
}

/// The lineage of each pending model, traced on `pool`.
fn trace_pass(
    request: &RichLineageRequest,
    pending: &[usize],
    (dialect, context): (Dialect, LineageContext<'_>),
    pool: &ThreadPool,
) -> Result<Vec<TracedModel>, String> {
    pool.install(|| {
        pending
            .par_iter()
            .map(|index| {
                catch_compiler_panic(|| {
                    Ok((
                        *index,
                        model_lineage(&request.models[*index], dialect, context),
                    ))
                })
            })
            .collect::<Result<Vec<_>, String>>()
    })
}
