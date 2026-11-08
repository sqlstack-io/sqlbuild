//! Fast lineage of a model without facts, read by its dialect's native analysis module.

use std::collections::HashMap;

use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;
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
use crate::semantic_validation::main::normalize_native::normalize_native_sql;

/// Lineage over `schema` and the `(name, type, resource)` references, or why it is unreadable.
pub fn fast_lineage(
    dialect: &dyn NativeDialect,
    (query_sql, placeholders): (&str, &[(String, String)]),
    references: &[(String, String, String)],
    schema: &[LineageSchemaResource],
) -> FastLineageOutcome {
    let normalised: String =
        match normalize_native_sql(query_sql, dialect.name(), dialect.stub_prefix()) {
            Ok(normalised) => normalised,
            Err(reason) => return FastLineageOutcome::Unparsed(reason),
        };
    let sql: String = placeholder_defaults(&normalised, placeholders);
    let relations: SessionSchema = names_schema(schema);
    let analysed: Result<NativeAnalysis, ParserGap> = dialect.analyse(&AnalysisInput {
        sql: &sql,
        schema: &relations,
        references,
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

/// Each resource with known columns, untyped and of unknown nullability; the first of a name wins.
fn names_schema(schema: &[LineageSchemaResource]) -> SessionSchema {
    let mut relations: HashMap<String, Vec<SchemaColumn>> = HashMap::new();
    for resource in schema
        .iter()
        .filter(|resource| !resource.columns.is_empty())
    {
        let _ = relations.entry(resource.name.clone()).or_insert_with(|| {
            resource
                .columns
                .iter()
                .map(|name| SchemaColumn {
                    name: name.clone(),
                    quoted: false,
                    data_type: None,
                    nullability: Nullability::Unknown,
                })
                .collect()
        });
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
