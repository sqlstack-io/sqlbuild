use std::collections::HashMap;

use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};

use crate::analysis_route::models::{AnalysisInput, NativeAnalysis, StandInSchema};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::{LineageRow, SessionSchema};
use crate::assembly::analysis_session::types::Shapes;
use crate::lineage::models::{LineageColumn, LineageSource};

/// `orders` with a non-null INTEGER id and a nullable VARCHAR status.
pub(super) fn orders() -> StandInSchema {
    StandInSchema::new(
        "orders",
        &[("id", "INTEGER", false), ("status", "VARCHAR", true)],
    )
}

/// The analysis of `sql` over `orders`, which the model references as the model `shop.orders`.
pub(super) fn analysed(
    dialect: &dyn NativeDialect,
    sql: &str,
    extension_functions: &[&str],
) -> NativeAnalysis {
    let schema: StandInSchema = orders();
    let references: [(String, String, String); 1] = [(
        "orders".to_owned(),
        "model".to_owned(),
        "shop.orders".to_owned(),
    )];
    let extension_functions: Vec<String> = extension_functions
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    dialect
        .analyse(&AnalysisInput {
            sql,
            schema: &schema,
            references: &references,
            extension_functions: &extension_functions,
        })
        .expect("the test query is read whole")
}

/// The columns a lineage row reads, each of which must come from the model `shop.orders`.
pub(super) fn resource_columns(row: &LineageRow) -> Vec<&str> {
    row.sources
        .iter()
        .map(|(resource_type, resource_name, column)| {
            assert_eq!(
                (resource_type.as_str(), resource_name.as_str()),
                ("model", "shop.orders"),
                "a source of {}",
                row.output_column
            );
            column.as_str()
        })
        .collect()
}

/// `output transform confidence [type:resource.column, ...]`.
pub(super) fn column_line(column: &LineageColumn) -> String {
    let sources: Vec<String> = column.upstream_columns.iter().map(source_text).collect();
    format!(
        "{} {} {} [{}]",
        column.output_column,
        column.transform_kind.as_str(),
        column.confidence.as_str(),
        sources.join(", ")
    )
}

fn source_text(source: &LineageSource) -> String {
    format!(
        "{}:{}.{}",
        source.resource_type.as_str(),
        source.resource_name,
        source.column_name
    )
}

/// `raw_orders` with `shape` as the session's input shapes and its typed schema; none when empty.
pub(super) fn session_relations(shape: &[(&str, &str, Nullability)]) -> (Shapes, SessionSchema) {
    let pairs: Vec<(String, String)> = shape
        .iter()
        .map(|(name, data_type, _)| ((*name).to_owned(), (*data_type).to_owned()))
        .collect();
    let columns: Vec<SchemaColumn> = shape.iter().map(schema_column).collect();
    let relations: HashMap<String, Vec<SchemaColumn>> = std::iter::once(("raw_orders", columns))
        .filter(|(_, columns)| !columns.is_empty())
        .map(|(name, columns)| (name.to_owned(), columns))
        .collect();
    (
        vec![("raw_orders".to_owned(), pairs)],
        SessionSchema { relations },
    )
}

fn schema_column(&(name, data_type, nullability): &(&str, &str, Nullability)) -> SchemaColumn {
    SchemaColumn {
        name: name.to_owned(),
        quoted: false,
        data_type: Some(data_type.to_owned()),
        nullability,
    }
}
