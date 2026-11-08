//! Fast lineage of a model without facts, read by its dialect's native analysis module.

use std::collections::HashMap;

use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;
use sqlbuild_sqltext::sql_scan::main::dialect_lexical_syntax::dialect_lexical_syntax;
use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};

use crate::analysis_route::_helpers::located_gap::{located, unreadable_reason};
use crate::analysis_route::models::{AnalysisInput, NativeAnalysis, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::constants::{
    CONFIDENCE_HIGH, CONFIDENCE_MEDIUM, TRANSFORM_AGGREGATION, TRANSFORM_CAST, TRANSFORM_CONSTANT,
    TRANSFORM_DIRECT, TRANSFORM_STAR,
};
use crate::assembly::analysis_session::models::{LineageRow, SessionSchema};
use crate::lineage::models::{
    FastLineageOutcome, LineageColumn, LineageConfidence, LineageResourceType,
    LineageSchemaResource, LineageSource, LineageTransformKind,
};
use crate::relation_markers::constants::MARKER_NAME_PREFIX;
use crate::relation_markers::main::markers_as_names::markers_as_names;
use crate::relation_markers::types::MarkerCall;
use crate::semantic_validation::main::normalize_native::normalize_native_sql;

/// Lineage over `schema` of `query_sql`, its relation markers read as names, or why it is unreadable.
pub fn fast_lineage(
    dialect: &dyn NativeDialect,
    (query_sql, placeholders): (&str, &[(String, String)]),
    schema: &[LineageSchemaResource],
) -> FastLineageOutcome {
    let (named, calls): (String, Vec<MarkerCall>) =
        match markers_as_names(query_sql, &dialect_lexical_syntax(dialect.name())) {
            Ok(named) => named,
            Err(reason) => return FastLineageOutcome::Unparsed(reason),
        };
    let normalised: String =
        match normalize_native_sql(&named, dialect.name(), dialect.stub_prefix()) {
            Ok(normalised) => normalised,
            Err(reason) => return FastLineageOutcome::Unparsed(reason),
        };
    let sql: String = placeholder_defaults(&normalised, placeholders);
    let referenced: Vec<(String, LineageResourceType, String)> = calls
        .into_iter()
        .enumerate()
        .filter_map(|(index, (function, name))| {
            let resource_type: LineageResourceType = match function.as_str() {
                "__ref" => LineageResourceType::Model,
                "__source" => LineageResourceType::Source,
                "__seed" => LineageResourceType::Seed,
                _ => return None,
            };
            Some((format!("{MARKER_NAME_PREFIX}{index}"), resource_type, name))
        })
        .collect();
    let references: Vec<(String, String, String)> = referenced
        .iter()
        .map(|(marker, resource_type, name)| {
            (
                marker.clone(),
                resource_type.as_str().to_owned(),
                name.clone(),
            )
        })
        .collect();
    let relations: SessionSchema = markers_schema(&referenced, schema);
    let analysed: Result<NativeAnalysis, ParserGap> = dialect.analyse(&AnalysisInput {
        sql: &sql,
        schema: &relations,
        references: &references,
        extension_functions: &[],
    });
    match analysed {
        Ok(native) => FastLineageOutcome::Built {
            columns: native.lineage.iter().map(lineage_column).collect(),
            has_star: native.has_star,
        },
        Err(gap) => {
            FastLineageOutcome::Unparsed(unreadable_reason(dialect.name(), &located(&sql, gap)))
        }
    }
}

/// Each marker name's resource columns, untyped and of unknown nullability, when the schema knows them.
fn markers_schema(
    referenced: &[(String, LineageResourceType, String)],
    schema: &[LineageSchemaResource],
) -> SessionSchema {
    let mut relations: HashMap<String, Vec<SchemaColumn>> = HashMap::new();
    for (marker, resource_type, name) in referenced {
        let Some(resource) = schema.iter().find(|resource| {
            resource.resource_type == *resource_type
                && resource.name == *name
                && !resource.columns.is_empty()
        }) else {
            continue;
        };
        let _ = relations.insert(
            marker.clone(),
            resource
                .columns
                .iter()
                .map(|column| SchemaColumn {
                    name: column.clone(),
                    quoted: false,
                    data_type: None,
                    nullability: Nullability::Unknown,
                })
                .collect(),
        );
    }
    SessionSchema { relations }
}

/// A session lineage row as a fast lineage column; sources of other resource kinds are dropped.
fn lineage_column(row: &LineageRow) -> LineageColumn {
    LineageColumn {
        output_column: row.output_column.clone(),
        transform_kind: match row.transform_code {
            TRANSFORM_DIRECT => LineageTransformKind::Direct,
            TRANSFORM_CAST => LineageTransformKind::Cast,
            TRANSFORM_AGGREGATION => LineageTransformKind::Aggregation,
            TRANSFORM_STAR => LineageTransformKind::Star,
            TRANSFORM_CONSTANT => LineageTransformKind::Constant,
            _ => LineageTransformKind::Expression,
        },
        confidence: match row.confidence_code {
            CONFIDENCE_HIGH => LineageConfidence::High,
            CONFIDENCE_MEDIUM => LineageConfidence::Medium,
            _ => LineageConfidence::Unknown,
        },
        upstream_columns: row
            .sources
            .iter()
            .filter_map(|(resource_type, resource_name, column_name)| {
                Some(LineageSource {
                    resource_type: LineageResourceType::from_value(resource_type)?,
                    resource_name: resource_name.clone(),
                    column_name: column_name.clone(),
                })
            })
            .collect(),
    }
}
