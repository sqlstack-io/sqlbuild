use polyglot_sql::{DialectType, SchemaValidationOptions};

use crate::lineage::main::build_fast_lineage::build_fast_lineage;
use crate::lineage::main::build_rich_lineage::build_rich_lineage;
use crate::lineage::models::RelationFingerprint;
use crate::lineage::models::{
    FastLineageModel, FastLineageOutcome, FastLineageRequest, LineageColumn, LineageOutcomeParts,
    LineageResourceType, LineageSchemaResource, RichLineageColumn, RichLineageRequest,
    RichSchemaResource,
};
use crate::semantic_validation::models::ProjectCatalog;

pub(crate) fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// Build one model against a fixed schema: `(status, column lines, has_star, detail)`.
pub(crate) fn outcome_lines(
    dialect: Option<&str>,
    model: FastLineageModel,
) -> Vec<(&'static str, Vec<String>, bool, Option<String>)> {
    let request = FastLineageRequest {
        dialect: dialect.map(str::to_owned),
        schema: schema(),
        models: vec![model],
    };
    build_fast_lineage(&request, Some(&catalog())).map_or_else(
        |error| vec![("error", Vec::new(), false, Some(error))],
        |outcomes| outcomes.into_iter().map(described_outcome).collect(),
    )
}

/// A `branches`-deep `UNION ALL` chain built from a caller thread with a `stack_bytes` stack.
pub(crate) fn deep_union_outcome_lines(
    branches: usize,
    stack_bytes: usize,
) -> Vec<(&'static str, Vec<String>, bool, Option<String>)> {
    let sql: String =
        vec!["SELECT order_id, amount FROM __ref('orders')"; branches].join(" UNION ALL ");
    let worker = std::thread::Builder::new()
        .stack_size(stack_bytes)
        .spawn(move || {
            outcome_lines(
                Some("duckdb"),
                FastLineageModel::Parse {
                    query_sql: sql,
                    inferred_columns: Vec::new(),
                    placeholders: Vec::new(),
                },
            )
        })
        .expect("the test thread starts");
    worker
        .join()
        .expect("the stage does not overflow the caller's stack")
}

fn catalog() -> ProjectCatalog {
    ProjectCatalog::with_options(
        DialectType::DuckDB,
        SchemaValidationOptions::default(),
        false,
    )
}

fn described_outcome(
    outcome: FastLineageOutcome,
) -> (&'static str, Vec<String>, bool, Option<String>) {
    let LineageOutcomeParts {
        status,
        columns,
        has_star,
        detail,
    } = outcome.into_parts();
    (status, described(&columns), has_star, detail)
}

fn described(columns: &[LineageColumn]) -> Vec<String> {
    columns.iter().map(described_column).collect()
}

fn described_column(column: &LineageColumn) -> String {
    let upstream: Vec<String> = column
        .upstream_columns
        .iter()
        .map(|source| {
            format!(
                "{}:{}.{}",
                source.resource_type.as_str(),
                source.resource_name,
                source.column_name
            )
        })
        .collect();
    format!(
        "{} {} {} [{}]",
        column.output_column,
        column.transform_kind.as_str(),
        column.confidence.as_str(),
        upstream.join(" ")
    )
}

fn schema() -> Vec<LineageSchemaResource> {
    vec![
        resource(
            LineageResourceType::Model,
            "orders",
            &["order_id", "customer_id", "order_id", "amount"],
        ),
        resource(
            LineageResourceType::Model,
            "customers",
            &["customer_id", "name"],
        ),
        resource(LineageResourceType::Model, "empty", &[]),
        resource(
            LineageResourceType::Seed,
            "regions",
            &["region_id", "label"],
        ),
    ]
}

fn resource(
    resource_type: LineageResourceType,
    name: &str,
    columns: &[&str],
) -> LineageSchemaResource {
    LineageSchemaResource {
        resource_type,
        name: name.to_owned(),
        columns: strings(columns),
    }
}

/// Build one model against a fixed typed schema: `(status, column lines, has_star, detail)`.
pub(crate) fn rich_outcome(
    dialect: &str,
    sql: &str,
) -> (&'static str, Vec<String>, bool, Option<String>) {
    let request = RichLineageRequest {
        dialect: dialect.to_owned(),
        schema: rich_schema(),
        models: vec![sql.to_owned()],
    };
    let mut outcomes = build_rich_lineage(&request).expect("the rich lineage pool builds");
    assert_eq!(outcomes.len(), 1);
    let LineageOutcomeParts {
        status,
        columns,
        has_star,
        detail,
    } = outcomes.remove(0).into_parts();
    (
        status,
        columns.iter().map(described_rich_column).collect(),
        has_star,
        detail,
    )
}

fn described_rich_column(column: &RichLineageColumn) -> String {
    let upstream: Vec<String> = column
        .column
        .upstream_columns
        .iter()
        .map(|source| {
            format!(
                "{}:{}.{}",
                source.resource_type.as_str(),
                source.resource_name,
                source.column_name
            )
        })
        .collect();
    format!(
        "{} {} {} {} [{}]",
        column.column.output_column,
        column.column.transform_kind.as_str(),
        column.column.confidence.as_str(),
        column.nullability.as_str(),
        upstream.join(" ")
    )
}

/// The typed schema `wheel_cases.py` gave the wheel, spelled through assigned and defaulted columns.
fn rich_schema() -> Vec<RichSchemaResource> {
    vec![
        RichSchemaResource {
            resource_type: LineageResourceType::Model,
            name: "orders".to_owned(),
            assigned: typed(&[
                ("order_id", Some("INTEGER")),
                ("customer_id", Some("TEXT")),
                ("customer_id", Some("INTEGER")),
                ("amount", Some("DOUBLE")),
            ]),
            defaulted: typed(&[("amount", Some("VARCHAR")), ("order_id", None)]),
        },
        RichSchemaResource {
            resource_type: LineageResourceType::Model,
            name: "customers".to_owned(),
            assigned: typed(&[("customer_id", Some("INTEGER"))]),
            defaulted: typed(&[("name", Some(""))]),
        },
        RichSchemaResource {
            resource_type: LineageResourceType::Model,
            name: "empty".to_owned(),
            assigned: Vec::new(),
            defaulted: Vec::new(),
        },
        RichSchemaResource {
            resource_type: LineageResourceType::Seed,
            name: "regions".to_owned(),
            assigned: typed(&[("region_id", Some("INTEGER")), ("label", Some("VARCHAR"))]),
            defaulted: Vec::new(),
        },
    ]
}

fn typed(columns: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
    columns
        .iter()
        .map(|(name, column_type)| ((*name).to_owned(), column_type.map(str::to_owned)))
        .collect()
}

/// `digest`, `uncacheable` or `failed: <message>` for a fingerprint outcome.
pub(crate) fn fingerprint_status(outcome: RelationFingerprint) -> String {
    outcome.into_digest().map_or_else(
        |message| format!("failed: {message}"),
        |digest| digest.map_or("uncacheable", |_| "digest").to_owned(),
    )
}
