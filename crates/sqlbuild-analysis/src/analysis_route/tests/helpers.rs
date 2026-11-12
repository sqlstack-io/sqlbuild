use std::collections::HashMap;

use sqlbuild_attachments::cursor_intrinsics::main::replace_intrinsics_with_spans::replace_intrinsics_with_spans;
use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};

use crate::analysis_route::_helpers::analysis::native_outcome::{
    analysed_sql, native_outcome, native_sql,
};
use crate::analysis_route::main::query_expression_ranges::query_expression_ranges;
use crate::analysis_route::main::schema_column::schema_column as stored_column;
use crate::analysis_route::models::DuckDbDialect;
use crate::analysis_route::models::{AnalysisInput, NativeAnalysis, StandInSchema};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::{
    LineageFacts, LineageRow, ModelAnalysis, ModelOutcome, ModelRequest, SessionFunctions,
    SessionSchema,
};
use crate::assembly::analysis_session::types::Shapes;
use crate::lineage::models::{LineageColumn, LineageSource};
use crate::semantic_checks::types::RevisedBinding;
use crate::semantic_validation::main::normalize_native_filled::normalize_native_filled;
use crate::sql_macros::main::typed_analysis_sql::typed_analysis_sql;
use crate::sql_macros::models::{TextSplice, TypedCall};

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
    let analysed: Result<NativeAnalysis, String> = cleaned.clone().and_then(|sql| {
        analysed_sql(
            dialect,
            &sql,
            (relations, &model.lineage_references),
            settings,
        )
    });
    native_outcome(model, schema, cleaned.unwrap_or_default(), analysed)
}

/// Each output column with the `query_sql` text its range covers after the compile passes.
pub(super) fn mapped_texts(
    query_sql: &str,
    placeholders: &[(&str, &str)],
    typed_calls: &[(&str, &str)],
) -> Vec<(String, Option<String>)> {
    let placeholders: Vec<(String, String)> = placeholders
        .iter()
        .map(|(name, text)| ((*name).to_owned(), (*text).to_owned()))
        .collect();
    let calls: Vec<TypedCall> = typed_calls
        .iter()
        .map(|(call, returns)| typed_call(query_sql, call, returns))
        .collect();
    let (typed, typed_splices) = typed_analysis_sql(query_sql, &calls, "duckdb");
    let (analysis_sql, spans) = replace_intrinsics_with_spans(&typed, "Model SQL", |_| {
        "CAST('2000-01-01' AS TIMESTAMP)".to_owned()
    })
    .expect("the cursor calls are replaced");
    let cursor_splices: Vec<TextSplice> = spans
        .iter()
        .map(|span| TextSplice {
            source: (span.source_start, span.source_end),
            output: (span.output_start, span.output_end),
        })
        .collect();
    let dialect: &DuckDbDialect = &DuckDbDialect;
    let (cleaned, _) = normalize_native_filled(
        &analysis_sql,
        "duckdb",
        dialect.stub_prefix(),
        &placeholders,
    )
    .expect("the test query normalises");
    let analysis: NativeAnalysis = analysed(dialect, &cleaned, &[]);
    let ranges: Vec<Option<(usize, usize)>> = analysis
        .lineage
        .iter()
        .map(|row| row.expression_range)
        .collect();
    let mapped = query_expression_ranges(
        "duckdb",
        (&analysis_sql, &placeholders),
        &[typed_splices, cursor_splices],
        &ranges,
    );
    analysis
        .lineage
        .iter()
        .zip(mapped)
        .map(|(row, range)| {
            let text: Option<String> = range.map(|(start, end)| query_sql[start..end].to_owned());
            (row.output_column.clone(), text)
        })
        .collect()
}

/// The typed call of the expression macro `call` in `query_sql`, returning `returns`.
fn typed_call(query_sql: &str, call: &str, returns: &str) -> TypedCall {
    let start: usize = query_sql.find(call).expect("the call is in the query");
    TypedCall {
        start,
        end: start + call.len(),
        macro_name: "typed_call".to_owned(),
        returns: Some(returns.to_owned()),
        returns_like: None,
    }
}
