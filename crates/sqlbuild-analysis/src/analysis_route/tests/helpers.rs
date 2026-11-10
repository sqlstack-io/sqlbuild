use std::collections::HashMap;

use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};

use crate::analysis_route::main::model_outcome::{analysed_sql, native_outcome, native_sql};
use crate::analysis_route::main::schema_column::schema_column as stored_column;
use crate::analysis_route::models::{AnalysisInput, NativeAnalysis, StandInSchema};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::{
    LineageFacts, LineageRow, ModelAnalysis, ModelOutcome, ModelRequest, SessionFunctions,
    SessionSchema,
};
use crate::assembly::analysis_session::types::Shapes;
use crate::lineage::models::{LineageColumn, LineageSource};
use crate::semantic_checks::types::RevisedBinding;

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
            quoted_ignore_case: false,
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
        SessionSchema {
            relations,
            open: HashMap::new(),
            functions: SessionFunctions::default(),
        },
    )
}

fn schema_column(&(name, data_type, nullability): &(&str, &str, Nullability)) -> SchemaColumn {
    stored_column(name, Some(data_type.to_owned()), nullability)
}

/// A finished outcome of `sql` over `orders`, as the session bound it.
pub(super) fn outcome(sql: &str) -> ModelOutcome {
    ModelOutcome {
        analysis: ModelAnalysis {
            analysis_succeeded: true,
            columns: None,
            lineage: LineageFacts::Native(Vec::new()),
            has_star: false,
            star_resolved: false,
            binding_diagnostics: Vec::new(),
            binding_validated: true,
        },
        cleaned_sql: sql.to_owned(),
        deferred_cleaning: None,
        validated_schema: vec![(
            "orders".to_owned(),
            vec![
                ("id".to_owned(), "INTEGER".to_owned()),
                ("status".to_owned(), "VARCHAR".to_owned()),
            ],
        )],
        fused_binding_validated: true,
        sql_macro_findings: Vec::new(),
    }
}

/// Each revised finding of every model as `(code, start, end)`, in order.
pub(super) fn finding_rows(
    models: &[Vec<RevisedBinding>],
) -> Vec<(&str, Option<i64>, Option<i64>)> {
    let mut rows: Vec<(&str, Option<i64>, Option<i64>)> = Vec::new();
    for (code, start, end) in models.iter().flatten() {
        rows.push((code.as_str(), *start, *end));
    }
    rows
}

/// The model's outcome over `schema` with the adapter's declared functions, and why it failed.
pub(crate) fn model_outcome(
    dialect: &dyn NativeDialect,
    model: &ModelRequest,
    (schema, relations): (Shapes, &SessionSchema),
    settings: (&[String], bool),
) -> (ModelOutcome, Option<String>) {
    let cleaned: Result<String, String> = native_sql(dialect, model, &HashMap::new());
    let analysed: Result<NativeAnalysis, String> = match &cleaned {
        Ok(sql) => analysed_sql(
            dialect,
            sql,
            (relations, &model.lineage_references),
            settings,
        ),
        Err(reason) => Err(reason.clone()),
    };
    native_outcome(model, schema, cleaned.unwrap_or_default(), analysed)
}
