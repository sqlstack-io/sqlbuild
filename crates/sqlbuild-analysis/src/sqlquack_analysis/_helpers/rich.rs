//! One model's rich-mode lineage from SQLQuack, read by physical resource names.

use std::collections::{BTreeSet, HashMap};

use rayon::ThreadPool;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{
    LineageContext, Nullability, OutputLineage, SchemaColumn,
};
use sqlquack_semantics::query_lineage;
use sqlquack_syntax::Dialect;

use crate::lineage::constants::{RICH_LINEAGE_WORKER_STACK_BYTES, RICH_LINEAGE_WORKERS};
use crate::lineage::main::model_references::model_references;
use crate::lineage::main::physical_name::physical_name;
use crate::lineage::models::{
    LineageColumn, LineageNullability, LineageResourceType, LineageSource, PhysicalResource,
    RichLineageColumn, RichLineageOutcome, RichLineageRequest,
};
use crate::sqlquack_analysis::_helpers::outputs::{
    confidence, lineage_parse_options, transform_kind,
};
use crate::sqlquack_analysis::_helpers::schema::{RelationColumns, schema_columns};
use crate::sqlquack_analysis::constants::TABLE_FUNCTION_CALL;
use crate::sqlquack_analysis::models::LearningPass;
use crate::sqlquack_analysis::types::TracedModel;

/// A model's outcome, and its output columns as later models read them.
pub(crate) fn model_lineage(
    query_sql: &str,
    dialect: Dialect,
    context: LineageContext<'_>,
) -> (RichLineageOutcome, Vec<SchemaColumn>) {
    let (normalized, references) = model_references(query_sql);
    let (normalized, functions) = table_function_calls(&normalized);
    let lineage = match query_lineage(&normalized, dialect, &lineage_parse_options(), context) {
        Ok(lineage) => lineage,
        Err(error) => return (RichLineageOutcome::Skipped(error), Vec::new()),
    };
    let resources: HashMap<String, PhysicalResource> = references
        .into_iter()
        .chain(functions)
        .map(|resource| {
            (
                context
                    .profile
                    .resolved_identifier(&resource.physical_name, false),
                resource,
            )
        })
        .collect();
    let named: Vec<&OutputLineage> = lineage
        .outputs
        .iter()
        .filter(|output| !output.name.is_empty())
        .collect();
    let columns: Vec<RichLineageColumn> = named
        .iter()
        .map(|output| RichLineageColumn {
            column: lineage_column(output, &resources),
            nullability: LineageNullability::Unknown,
        })
        .collect();
    let outputs: Vec<SchemaColumn> = named
        .iter()
        .map(|output| SchemaColumn {
            name: output.name.clone(),
            quoted: context.profile.resolved_identifier(&output.name, false) != output.key,
            data_type: None,
            nullability: Nullability::Unknown,
        })
        .collect();
    (
        RichLineageOutcome::Built {
            columns,
            has_star: lineage.open_star,
        },
        outputs,
    )
}

/// The request schema's columns by resolved physical name, for resources that list any.
pub(crate) fn known_columns(
    request: &RichLineageRequest,
    profile: &dyn DialectProfile,
) -> HashMap<String, Vec<SchemaColumn>> {
    let mut known: HashMap<String, Vec<SchemaColumn>> = HashMap::new();
    for resource in &request.schema {
        let names = resource.assigned.iter().chain(&resource.defaulted);
        let columns: Vec<SchemaColumn> = schema_columns(names.map(|(name, _)| name.as_str()));
        if columns.is_empty() {
            continue;
        }
        let physical: String = physical_name(resource.resource_type, &resource.name);
        let _ = known.insert(profile.resolved_identifier(&physical, false), columns);
    }
    known
}

/// Each named model's resolved physical name.
pub(crate) fn model_keys(names: &[String], profile: &dyn DialectProfile) -> Vec<String> {
    names
        .iter()
        .map(|name| {
            profile.resolved_identifier(&physical_name(LineageResourceType::Model, name), false)
        })
        .collect()
}

/// The worker pool rich lineage traces `models` models on.
pub(crate) fn lineage_pool(models: usize) -> Result<ThreadPool, String> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(RICH_LINEAGE_WORKERS.min(models.max(1)))
        .stack_size(RICH_LINEAGE_WORKER_STACK_BYTES)
        .build()
        .map_err(|error| error.to_string())
}

/// Trace the pending models against the known columns; what they teach, and who reads it.
pub(crate) fn learning_pass(
    (request, keys): (&RichLineageRequest, &[String]),
    (known, pending): (&HashMap<String, Vec<SchemaColumn>>, &[usize]),
    (dialect, profile): (Dialect, &'static dyn DialectProfile),
    pool: &ThreadPool,
) -> Result<LearningPass, String> {
    let schema = RelationColumns(known.clone());
    let context = LineageContext {
        profile,
        schema: &schema,
        sql: None,
    };
    let traced: Vec<TracedModel> = trace_pass(request, pending, (dialect, context), pool)?;
    let learnt: Vec<(String, Vec<SchemaColumn>)> = newly_learnt(&traced, keys, known);
    let learnt_keys: Vec<String> = learnt.iter().map(|(key, _)| key.clone()).collect();
    let readers: Vec<usize> = readers(request, &learnt_keys, context);
    Ok(LearningPass {
        traced,
        learnt,
        readers,
    })
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

/// The models whose lineage found all their outputs, by key, where those outputs are new.
fn newly_learnt(
    traced: &[TracedModel],
    keys: &[String],
    known: &HashMap<String, Vec<SchemaColumn>>,
) -> Vec<(String, Vec<SchemaColumn>)> {
    let mut learnt: Vec<(String, Vec<SchemaColumn>)> = Vec::new();
    for (index, (outcome, outputs)) in traced {
        let complete: bool = matches!(
            outcome,
            RichLineageOutcome::Built {
                has_star: false,
                ..
            }
        );
        let key: &String = &keys[*index];
        if complete && !outputs.is_empty() && known.get(key) != Some(outputs) {
            learnt.push((key.clone(), outputs.clone()));
        }
    }
    learnt
}

/// The models that read any of the `learnt` resources, by resolved physical name.
fn readers(
    request: &RichLineageRequest,
    learnt: &[String],
    context: LineageContext<'_>,
) -> Vec<usize> {
    let mut readers: Vec<usize> = Vec::new();
    for (index, sql) in request.models.iter().enumerate() {
        let (_, references) = model_references(sql);
        let reads: bool = references.iter().any(|resource| {
            learnt.contains(
                &context
                    .profile
                    .resolved_identifier(&resource.physical_name, false),
            )
        });
        if reads {
            readers.push(index);
        }
    }
    readers
}

fn lineage_column(
    output: &OutputLineage,
    resources: &HashMap<String, PhysicalResource>,
) -> LineageColumn {
    let mut upstream: BTreeSet<(&'static str, String, String)> = BTreeSet::new();
    let mut types: HashMap<(&'static str, String), LineageResourceType> = HashMap::new();
    for source in &output.sources {
        let Some(resource) = resources.get(&source.relation) else {
            continue;
        };
        let kind: &'static str = resource.resource_type.as_str();
        let _ = types.insert(
            (kind, resource.resource_name.clone()),
            resource.resource_type,
        );
        let _ = upstream.insert((kind, resource.resource_name.clone(), source.column.clone()));
    }
    let upstream_columns: Vec<LineageSource> = upstream
        .into_iter()
        .filter_map(|(kind, name, column)| {
            Some(LineageSource {
                resource_type: *types.get(&(kind, name.clone()))?,
                resource_name: name,
                column_name: column,
            })
        })
        .collect();
    LineageColumn {
        output_column: output.name.clone(),
        transform_kind: transform_kind(output.kind),
        confidence: confidence(output.resolved, !upstream_columns.is_empty()),
        upstream_columns,
    }
}

/// `sql` with each `__table_fn("name")(` written as a call of its physical name, and those calls.
fn table_function_calls(sql: &str) -> (String, Vec<PhysicalResource>) {
    let mut text = String::with_capacity(sql.len());
    let mut functions: Vec<PhysicalResource> = Vec::new();
    let mut rest = sql;
    while let Some(start) = rest.find(TABLE_FUNCTION_CALL) {
        text.push_str(&rest[..start]);
        let after = &rest[start + TABLE_FUNCTION_CALL.len()..];
        match table_function_name(after) {
            Some((name, closed)) => {
                let physical_name = physical_name(LineageResourceType::TableFn, name);
                text.push_str(&physical_name);
                functions.push(PhysicalResource {
                    resource_type: LineageResourceType::TableFn,
                    resource_name: name.to_owned(),
                    physical_name,
                });
                rest = closed;
            }
            None => {
                text.push_str(TABLE_FUNCTION_CALL);
                rest = after;
            }
        }
    }
    text.push_str(rest);
    (text, functions)
}

/// The quoted name opening `after`, and the text after its closing parenthesis.
fn table_function_name(after: &str) -> Option<(&str, &str)> {
    let quoted = after.trim_start();
    let quote: char = quoted
        .chars()
        .next()
        .filter(|quote| *quote == '\'' || *quote == '"')?;
    let name_end = quoted[1..].find(quote)? + 1;
    let closed = quoted[name_end + 1..].trim_start().strip_prefix(')')?;
    Some((&quoted[1..name_end], closed))
}
