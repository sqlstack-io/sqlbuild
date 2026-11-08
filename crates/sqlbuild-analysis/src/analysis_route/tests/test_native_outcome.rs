use crate::analysis_route::main::fast_lineage::fast_lineage;
use crate::analysis_route::main::model_outcome::model_outcome;
use crate::analysis_route::tests::helpers::{column_line, session_relations};
use crate::analysis_route::tests::test_types::{
    CataloguedDialect, NativeLineageTestCase, NativeOutcomeTestCase, PivotlessDialect,
};
use crate::assembly::analysis_session::models::{ModelOutcome, ModelRequest, SessionSchema};
use crate::assembly::analysis_session::types::Shapes;
use crate::lineage::models::{
    LineageColumn, LineageOutcomeParts, LineageResourceType, LineageSchemaResource,
};
use sqlquack_semantics::lineage::models::Nullability;

#[test]
fn given_session_shapes_when_a_native_route_analyses_a_model_then_its_facts_follow_them() {
    let test_cases = [
        NativeOutcomeTestCase {
            description: "a closed source expands a star with its types and known nullability",
            sql: "SELECT * FROM raw_orders",
            shape: &[
                ("order_id", "INTEGER", Nullability::NonNull),
                ("status", "VARCHAR", Nullability::Nullable),
            ],
            declared_functions: &[],
            expected_lines: &[
                "order_id INTEGER non_null",
                "status VARCHAR nullable",
                "succeeded=true star=true/true findings=[]",
            ],
        },
        NativeOutcomeTestCase {
            description: "a relation the session has no shape for is unknown, not empty",
            sql: "SELECT * FROM raw_orders",
            shape: &[],
            declared_functions: &[],
            expected_lines: &["succeeded=true star=true/false findings=[]"],
        },
        NativeOutcomeTestCase {
            description: "SQL the dialect cannot read fails the analysis",
            sql: "SELECT FROM WHERE",
            shape: &[("order_id", "INTEGER", Nullability::NonNull)],
            declared_functions: &[],
            expected_lines: &["succeeded=false star=false/false findings=[]"],
        },
        NativeOutcomeTestCase {
            description: "a function the adapter declares is not unknown",
            sql: "SELECT CURRENT_TIMESTAMP() AS at FROM raw_orders",
            shape: &[("order_id", "INTEGER", Nullability::NonNull)],
            declared_functions: &[("CURRENT_TIMESTAMP", "TIMESTAMPTZ")],
            expected_lines: &[
                "at TIMESTAMPTZ unknown",
                "succeeded=true star=false/true findings=[]",
            ],
        },
        NativeOutcomeTestCase {
            description: "the same function without the declaration is unknown",
            sql: "SELECT CURRENT_TIMESTAMP() AS at FROM raw_orders",
            shape: &[("order_id", "INTEGER", Nullability::NonNull)],
            declared_functions: &[],
            expected_lines: &[
                "at TIMESTAMPTZ unknown",
                "succeeded=true star=false/true findings=[\"B101\"]",
            ],
        },
    ];

    for test_case in test_cases {
        let (schema, relations): (Shapes, SessionSchema) = session_relations(test_case.shape);
        let model: ModelRequest = ModelRequest {
            name: "orders".to_owned(),
            query_sql: test_case.sql.to_owned(),
            required_names: vec!["raw_orders".to_owned()],
            ..ModelRequest::default()
        };
        let declared: Vec<String> = test_case
            .declared_functions
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect();
        let (outcome, failure): (ModelOutcome, Option<String>) =
            model_outcome(&CataloguedDialect, &model, (schema, &relations), &declared);
        let analysis = &outcome.analysis;
        let mut lines: Vec<String> = analysis
            .columns
            .iter()
            .flatten()
            .map(|column| {
                format!(
                    "{} {} {}",
                    column.name,
                    column.data_type.as_deref().unwrap_or("-"),
                    column.nullability
                )
            })
            .collect();
        let findings: Vec<&str> = analysis
            .binding_diagnostics
            .iter()
            .map(|row| row.code.as_str())
            .collect();
        lines.push(format!(
            "succeeded={} star={}/{} findings={findings:?}",
            analysis.analysis_succeeded, analysis.has_star, analysis.star_resolved
        ));
        assert_eq!(lines, test_case.expected_lines, "{}", test_case.description);
        assert_eq!(
            failure.is_some(),
            !analysis.analysis_succeeded,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_model_without_facts_when_fast_lineage_reads_it_natively_then_it_has_lineage_or_a_reason()
{
    let schema: Vec<LineageSchemaResource> = vec![
        LineageSchemaResource {
            resource_type: LineageResourceType::Model,
            name: "orders".to_owned(),
            columns: vec!["order_id".to_owned(), "amount".to_owned()],
        },
        LineageSchemaResource {
            resource_type: LineageResourceType::Source,
            name: "raw_payments".to_owned(),
            columns: vec!["payment_id".to_owned()],
        },
    ];
    let references: Vec<(String, String, String)> = [
        ("orders", "model"),
        ("unknown_model", "model"),
        ("raw_payments", "source"),
    ]
    .iter()
    .map(|(name, kind)| ((*name).to_owned(), (*kind).to_owned(), (*name).to_owned()))
    .collect();
    let test_cases = [
        NativeLineageTestCase {
            description: "a referenced model's columns are the upstreams of direct and computed outputs",
            sql: r#"SELECT o.order_id, o.amount * 2 AS doubled FROM __ref("orders") AS o"#,
            placeholders: &[],
            expected_lines: &[
                "order_id direct high [model:orders.order_id]",
                "doubled expression high [model:orders.amount]",
                "built star=false",
            ],
        },
        NativeLineageTestCase {
            description: "a placeholder takes its default before the parse, and a source is a source",
            sql: r#"SELECT payment_id FROM __source("raw_payments") WHERE payment_id > @@@min_id"#,
            placeholders: &[("min_id", "0")],
            expected_lines: &[
                "payment_id direct high [source:raw_payments.payment_id]",
                "built star=false",
            ],
        },
        NativeLineageTestCase {
            description: "a star over a relation of unknown columns is a star without outputs",
            sql: r#"SELECT * FROM __ref("unknown_model")"#,
            placeholders: &[],
            expected_lines: &["built star=true"],
        },
        NativeLineageTestCase {
            description: "SQL the parser cannot read gives P026's reason, located",
            sql: "SELECT 1\nFROM t PIVOT (sum(x) FOR y IN (1))",
            placeholders: &[],
            expected_lines: &[
                "unparsed star=false",
                "the postgres parser cannot read PIVOT at line 2, column 8",
            ],
        },
    ];

    for test_case in test_cases {
        let placeholders: Vec<(String, String)> = test_case
            .placeholders
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        let parts: LineageOutcomeParts<LineageColumn> = fast_lineage(
            &PivotlessDialect,
            (test_case.sql, &placeholders),
            &references,
            &schema,
        )
        .into_parts();
        let mut lines: Vec<String> = parts.columns.iter().map(column_line).collect();
        lines.push(format!("{} star={}", parts.status, parts.has_star));
        lines.extend(parts.detail);
        assert_eq!(lines, test_case.expected_lines, "{}", test_case.description);
    }
}
