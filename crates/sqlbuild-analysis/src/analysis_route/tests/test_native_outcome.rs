use crate::analysis_route::tests::test_types::{
    CataloguedDialect, NativeLineageTestCase, NativeOutcomeTestCase, PivotlessDialect,
};
use crate::assembly::analysis_session::_helpers::mappings::ShapeTable;
use crate::assembly::analysis_session::_helpers::native_outcome::native_outcome;
use crate::assembly::analysis_session::models::{ModelOutcome, ModelRequest};
use crate::assembly::analysis_session::types::Shapes;
use crate::lineage::_helpers::native_lineage::native_model_lineage;
use crate::lineage::models::{FastLineageOutcome, LineageResourceType, LineageSchemaResource};

#[test]
fn given_session_shapes_when_a_native_route_analyses_a_model_then_its_facts_follow_them() {
    let test_cases = [
        NativeOutcomeTestCase {
            description: "a closed source expands a star with its types and known nullability",
            sql: "SELECT * FROM raw_orders",
            shape: &[("order_id", "INTEGER"), ("status", "VARCHAR")],
            declared_functions: &[],
            expected: &[
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
            expected: &["succeeded=true star=true/false findings=[]"],
        },
        NativeOutcomeTestCase {
            description: "SQL the dialect cannot read fails the analysis",
            sql: "SELECT FROM WHERE",
            shape: &[("order_id", "INTEGER")],
            declared_functions: &[],
            expected: &["succeeded=false star=false/false findings=[]"],
        },
        NativeOutcomeTestCase {
            description: "a function the adapter declares is not unknown",
            sql: "SELECT CURRENT_TIMESTAMP() AS at FROM raw_orders",
            shape: &[("order_id", "INTEGER")],
            declared_functions: &[("CURRENT_TIMESTAMP", "TIMESTAMPTZ")],
            expected: &[
                "at TIMESTAMPTZ unknown",
                "succeeded=true star=false/true findings=[]",
            ],
        },
        NativeOutcomeTestCase {
            description: "the same function without the declaration is unknown",
            sql: "SELECT CURRENT_TIMESTAMP() AS at FROM raw_orders",
            shape: &[("order_id", "INTEGER")],
            declared_functions: &[],
            expected: &[
                "at TIMESTAMPTZ unknown",
                "succeeded=true star=false/true findings=[\"B101\"]",
            ],
        },
    ];

    for test_case in test_cases {
        let shape: Vec<(String, String)> = test_case
            .shape
            .iter()
            .map(|(name, data_type)| ((*name).to_owned(), (*data_type).to_owned()))
            .collect();
        let schema: Shapes = vec![("raw_orders".to_owned(), shape)];
        let nullability: ShapeTable = ShapeTable::from_shapes(&vec![(
            "raw_orders".to_owned(),
            vec![
                ("order_id".to_owned(), "non_null".to_owned()),
                ("status".to_owned(), "nullable".to_owned()),
            ],
        )]);
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
        let (outcome, failure): (ModelOutcome, Option<String>) = native_outcome(
            &CataloguedDialect,
            &model,
            (schema, &nullability),
            &declared,
        );
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
        assert_eq!(lines, test_case.expected, "{}", test_case.description);
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
    let test_cases = [
        NativeLineageTestCase {
            description: "a referenced model's columns are the upstreams of direct and computed outputs",
            sql: r#"SELECT o.order_id, o.amount * 2 AS doubled FROM __ref("orders") AS o"#,
            placeholders: &[],
            expected: &[
                "order_id direct high [model:orders.order_id]",
                "doubled expression high [model:orders.amount]",
                "star=false",
            ],
        },
        NativeLineageTestCase {
            description: "a placeholder takes its default before the parse, and a source is a source",
            sql: r#"SELECT payment_id FROM __source("raw_payments") WHERE payment_id > @@@min_id"#,
            placeholders: &[("min_id", "0")],
            expected: &[
                "payment_id direct high [source:raw_payments.payment_id]",
                "star=false",
            ],
        },
        NativeLineageTestCase {
            description: "a star over a relation of unknown columns is a star without outputs",
            sql: r#"SELECT * FROM __ref("unknown_model")"#,
            placeholders: &[],
            expected: &["star=true"],
        },
        NativeLineageTestCase {
            description: "SQL the parser cannot read gives P026's reason, located",
            sql: "SELECT 1\nFROM t PIVOT (sum(x) FOR y IN (1))",
            placeholders: &[],
            expected: &["unparsed: the postgres parser cannot read PIVOT at line 2, column 8"],
        },
    ];

    for test_case in test_cases {
        let placeholders: Vec<(String, String)> = test_case
            .placeholders
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        let outcome: FastLineageOutcome =
            native_model_lineage(&PivotlessDialect, test_case.sql, &placeholders, &schema);
        let lines: Vec<String> = match outcome {
            FastLineageOutcome::Built { columns, has_star } => columns
                .iter()
                .map(|column| {
                    let sources: Vec<String> = column
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
                        sources.join(", ")
                    )
                })
                .chain([format!("star={has_star}")])
                .collect(),
            FastLineageOutcome::Unparsed(reason) => vec![format!("unparsed: {reason}")],
            other => vec![format!("{other:?}")],
        };
        assert_eq!(lines, test_case.expected, "{}", test_case.description);
    }
}
