use crate::analysis_route::models::{AnalysisInput, NativeAnalysis, StandInSchema};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::LineageRow;

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
