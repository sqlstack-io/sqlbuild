//! Fast lineage for unresolved root stars and models without compact facts.

use std::sync::Arc;

use polyglot_sql::ParseOptions;
use rayon::ThreadPool;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_core::panics::main::native_failure::native_failure;

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::lineage::_helpers::native_lineage::native_model_lineage;
use crate::lineage::_helpers::parsed_lineage::{
    ParsedModel, parsed_model_lineage, proxy_parse_options,
};
use crate::lineage::_helpers::references::physical_resources;
use crate::lineage::_helpers::stars::{schema_mapping, star_lineage};
use crate::lineage::main::parser_dialect::parser_dialect;
use crate::lineage::models::{FastLineageModel, FastLineageOutcome, FastLineageRequest};
use crate::semantic_validation::main::new_analysis_pool::new_analysis_pool;
use crate::semantic_validation::models::ProjectCatalog;

/// One outcome per model, in request order, run on the compile's deep-stack analysis pool.
pub fn build_fast_lineage(
    request: &FastLineageRequest,
    catalog: Option<&ProjectCatalog>,
) -> Result<Vec<FastLineageOutcome>, String> {
    let pool: Arc<ThreadPool> = match catalog {
        Some(catalog) => catalog.analysis_pool()?,
        None => Arc::new(new_analysis_pool()?),
    };
    let outcomes: Vec<Result<FastLineageOutcome, String>> =
        pool.install(|| model_outcomes(request));
    outcomes
        .into_iter()
        .enumerate()
        .map(|(index, outcome)| {
            outcome.map_err(|reason| {
                native_failure(
                    &format!("native fast lineage of request model {index}"),
                    &reason,
                )
            })
        })
        .collect()
}

/// Each model's outcome or internal failure, panics included, in request order.
fn model_outcomes(request: &FastLineageRequest) -> Vec<Result<FastLineageOutcome, String>> {
    let schema = schema_mapping(&request.schema);
    let name: &str = request
        .dialect
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or("generic");
    let dialect = parser_dialect(Some(name));
    let route: AnalysisRoute = analysis_route(Some(name));
    let options: Result<ParseOptions, serde_json::Error> = proxy_parse_options();
    request
        .models
        .par_iter()
        .map(|model| match (model, route) {
            (
                FastLineageModel::StarExpansion {
                    query_sql,
                    existing_columns,
                },
                _,
            ) => Ok(FastLineageOutcome::StarColumns(star_lineage(
                &schema,
                &physical_resources(query_sql),
                existing_columns.iter().map(String::as_str),
            ))),
            (
                FastLineageModel::Parse {
                    query_sql,
                    placeholders,
                    ..
                },
                AnalysisRoute::Native(module),
            ) => catch_compiler_panic(|| {
                Ok(native_model_lineage(
                    module,
                    query_sql,
                    placeholders,
                    &request.schema,
                ))
            }),
            (FastLineageModel::Parse { .. }, AnalysisRoute::Unavailable) => {
                Ok(FastLineageOutcome::Omitted)
            }
            (
                FastLineageModel::Parse {
                    query_sql,
                    inferred_columns,
                    ..
                },
                AnalysisRoute::Legacy,
            ) => {
                let model = ParsedModel {
                    query_sql,
                    inferred_names: inferred_columns,
                    schema: &schema,
                    dialect,
                    options: &options,
                };
                catch_compiler_panic(|| parsed_model_lineage(&model))
            }
        })
        .collect()
}
