//! One model's rich-mode lineage from SQLQuack, read by physical resource names.

use std::collections::{BTreeSet, HashMap};

use sqlquack_semantics::lineage::models::{LineageContext, OutputLineage, SchemaColumn};
use sqlquack_semantics::query_lineage;
use sqlquack_syntax::Dialect;

use crate::lineage::_helpers::references::{
    PhysicalResource, normalized_sql, physical_resource_name, physical_resources,
};
use crate::lineage::models::{
    LineageColumn, LineageNullability, LineageResourceType, LineageSource, RichLineageColumn,
    RichLineageOutcome,
};
use crate::sqlquack_analysis::_helpers::outputs::{
    confidence, lineage_parse_options, transform_kind,
};
use crate::sqlquack_analysis::constants::TABLE_FUNCTION_CALL;

/// A model's outcome, and its output columns as later models read them.
pub(crate) fn model_lineage(
    query_sql: &str,
    dialect: Dialect,
    context: LineageContext<'_>,
) -> (RichLineageOutcome, Vec<SchemaColumn>) {
    let (normalized, functions) = table_function_calls(&normalized_sql(query_sql));
    let lineage = match query_lineage(&normalized, dialect, &lineage_parse_options(), context) {
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

/// Whether `sql` reads any of the `learnt` resources, by resolved physical name.
pub(crate) fn reads_learnt(sql: &str, learnt: &[String], context: LineageContext<'_>) -> bool {
    physical_resources(sql).iter().any(|resource| {
        learnt.contains(
            &context
                .profile
                .resolved_identifier(&resource.physical_name, false),
        )
    })
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
