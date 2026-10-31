//! Rich column lineage from SQLQuack's syntax tree and lineage, for Snowflake and DuckDB.

use std::collections::{BTreeSet, HashMap};

use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlquack_semantics::dialects::constants::{DUCKDB, SNOWFLAKE};
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{
    LineageContext, OutputLineage, SchemaColumn, TransformKind,
};
use sqlquack_semantics::lineage::types::LineageSchema;
use sqlquack_semantics::query_lineage;
use sqlquack_syntax::{ComplexityGuardOptions, Dialect, ParseOptions};

use crate::lineage::_helpers::references::{
    PhysicalResource, normalized_sql, physical_resource_name, physical_resources,
};
use crate::lineage::_helpers::rich_lineage::schema_names;
use crate::lineage::constants::{
    MAX_FUNCTION_CALL_DEPTH, RICH_LINEAGE_WORKER_STACK_BYTES, RICH_LINEAGE_WORKERS,
};
use crate::lineage::models::{
    LineageColumn, LineageConfidence, LineageNullability, LineageResourceType, LineageSource,
    LineageTransformKind, RichLineageColumn, RichLineageOutcome, RichLineageRequest,
};

const DUCKDB_DIALECT: &str = "duckdb";
const SNOWFLAKE_DIALECT: &str = "snowflake";
const TABLE_FUNCTION_CALL: &str = "__table_fn(";
/// Passes of learning model columns from their own lineage; deeper chains stay open.
const MAX_LEARNING_PASSES: usize = 64;

/// The known resources' columns by resolved physical name.
#[derive(Debug)]
struct ResourceColumns(HashMap<String, Vec<SchemaColumn>>);

impl LineageSchema for ResourceColumns {
    fn columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        self.0.get(relation).map(Vec::as_slice)
    }
}

/// One outcome per model, in request order; `names` names each model of the request.
///
/// A model whose own lineage finds all its outputs is read by those outputs, exactly as they
/// resolve, rather than by the schema's names; the models reading it are traced again until
/// nothing more is learnt.
pub fn build_sqlquack_lineage(
    request: &RichLineageRequest,
    names: &[String],
) -> Result<Vec<RichLineageOutcome>, String> {
    let (dialect, profile): (Dialect, &'static dyn DialectProfile) = match request.dialect.as_str()
    {
        SNOWFLAKE_DIALECT => (Dialect::Snowflake, &SNOWFLAKE),
        DUCKDB_DIALECT => (Dialect::DuckDB, &DUCKDB),
        other => return Err(format!("SQLQuack does not analyse the {other} dialect")),
    };
    let mut known: HashMap<String, Vec<SchemaColumn>> = request
        .schema
        .iter()
        .map(schema_names)
        .filter(|resource| !resource.columns.is_empty())
        .map(|resource| {
            let physical = physical_resource_name(resource.resource_type, &resource.name);
            (
                profile.resolved_identifier(&physical, false),
                schema_columns(resource.columns),
            )
        })
        .collect();
    let model_keys: Vec<String> = names
        .iter()
        .map(|name| {
            profile.resolved_identifier(
                &physical_resource_name(LineageResourceType::Model, name),
                false,
            )
        })
        .collect();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(RICH_LINEAGE_WORKERS.min(request.models.len().max(1)))
        .stack_size(RICH_LINEAGE_WORKER_STACK_BYTES)
        .build()
        .map_err(|error| error.to_string())?;
    let mut outcomes: Vec<Option<RichLineageOutcome>> = vec![None; request.models.len()];
    let mut pending: Vec<usize> = (0..request.models.len()).collect();
    for _ in 0..MAX_LEARNING_PASSES {
        let schema = ResourceColumns(known.clone());
        let context = LineageContext {
            profile,
            schema: &schema,
            sql: None,
        };
        let traced: Vec<(usize, (RichLineageOutcome, Vec<SchemaColumn>))> = pool.install(|| {
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
        })?;
        let mut learnt: Vec<String> = Vec::new();
        for (index, (outcome, outputs)) in traced {
            let key = &model_keys[index];
            let complete = matches!(
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
            .filter(|index| {
                let sql = &request.models[*index];
                physical_resources(sql).iter().any(|resource| {
                    learnt.contains(&profile.resolved_identifier(&resource.physical_name, false))
                })
            })
            .collect();
        if pending.is_empty() {
            break;
        }
    }
    Ok(outcomes.into_iter().flatten().collect())
}

/// A model's outcome, and its output columns as later models read them.
fn model_lineage(
    query_sql: &str,
    dialect: Dialect,
    context: LineageContext<'_>,
) -> (RichLineageOutcome, Vec<SchemaColumn>) {
    let options = ParseOptions {
        complexity_guard: Some(ComplexityGuardOptions {
            max_function_call_depth: Some(MAX_FUNCTION_CALL_DEPTH),
            ..ComplexityGuardOptions::default()
        }),
    };
    let (normalized, functions) = table_function_calls(&normalized_sql(query_sql));
    let lineage = match query_lineage(&normalized, dialect, &options, context) {
        Ok(lineage) => lineage,
        Err(error) => return (RichLineageOutcome::Skipped(error), Vec::new()),
    };
    let resources: HashMap<String, PhysicalResource> = physical_resources(query_sql)
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
    let columns = lineage
        .outputs
        .iter()
        .filter(|output| !output.name.is_empty())
        .map(|output| RichLineageColumn {
            column: lineage_column(output, &resources),
            nullability: LineageNullability::Unknown,
        })
        .collect();
    let outputs = lineage
        .outputs
        .iter()
        .filter(|output| !output.name.is_empty())
        .map(|output| SchemaColumn {
            name: output.name.clone(),
            quoted: context.profile.resolved_identifier(&output.name, false) != output.key,
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

/// A resource's column names as the schema lists them: a name written in double quotes is a
/// quoted identifier, and so is the same name listed without them.
fn schema_columns(names: Vec<String>) -> Vec<SchemaColumn> {
    let quoted: Vec<&str> = names
        .iter()
        .filter_map(|name| name.strip_prefix('"')?.strip_suffix('"'))
        .collect();
    let mut columns: Vec<SchemaColumn> = Vec::with_capacity(names.len());
    for name in &names {
        let column = match name
            .strip_prefix('"')
            .and_then(|name| name.strip_suffix('"'))
        {
            Some(inner) => SchemaColumn {
                name: inner.to_owned(),
                quoted: true,
            },
            None => SchemaColumn {
                name: name.clone(),
                quoted: quoted.contains(&name.as_str()),
            },
        };
        if !columns.contains(&column) {
            columns.push(column);
        }
    }
    columns
}

fn lineage_column(
    output: &OutputLineage,
    resources: &HashMap<String, PhysicalResource>,
) -> LineageColumn {
    let upstream: BTreeSet<(&'static str, String, String)> = output
        .sources
        .iter()
        .filter_map(|source| {
            let resource = resources.get(&source.relation)?;
            Some((
                resource.resource_type.as_str(),
                resource.resource_name.clone(),
                source.column.clone(),
            ))
        })
        .collect();
    let upstream_columns: Vec<LineageSource> = upstream
        .into_iter()
        .filter_map(|(kind, name, column)| {
            let resource = resources.values().find(|resource| {
                resource.resource_type.as_str() == kind && resource.resource_name == name
            })?;
            Some(LineageSource {
                resource_type: resource.resource_type,
                resource_name: name,
                column_name: column,
            })
        })
        .collect();
    let transform_kind = match output.kind {
        TransformKind::Constant => LineageTransformKind::Constant,
        TransformKind::Direct => LineageTransformKind::Direct,
        TransformKind::Cast => LineageTransformKind::Cast,
        TransformKind::Expression => LineageTransformKind::Expression,
        TransformKind::Aggregation => LineageTransformKind::Aggregation,
    };
    let confidence = match (output.resolved, upstream_columns.is_empty(), transform_kind) {
        (true, _, _) => LineageConfidence::High,
        (false, false, _) => LineageConfidence::Medium,
        (false, true, _) => LineageConfidence::Unknown,
    };
    LineageColumn {
        output_column: output.name.clone(),
        transform_kind,
        confidence,
        upstream_columns,
    }
}

/// `sql` with each `__table_fn("name")(` written as a call of the function's physical name,
/// and the table functions it calls.
fn table_function_calls(sql: &str) -> (String, Vec<PhysicalResource>) {
    let mut text = String::with_capacity(sql.len());
    let mut functions: Vec<PhysicalResource> = Vec::new();
    let mut rest = sql;
    while let Some(start) = rest.find(TABLE_FUNCTION_CALL) {
        text.push_str(&rest[..start]);
        let after = &rest[start + TABLE_FUNCTION_CALL.len()..];
        let quoted = after.trim_start();
        let quote = quoted
            .chars()
            .next()
            .filter(|quote| *quote == '\'' || *quote == '"');
        let call = quote.and_then(|quote| {
            let name_end = quoted[1..].find(quote)? + 1;
            let closed = quoted[name_end + 1..].trim_start().strip_prefix(')')?;
            Some((&quoted[1..name_end], closed))
        });
        match call {
            Some((name, closed)) => {
                let physical_name = physical_resource_name(LineageResourceType::TableFn, name);
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
