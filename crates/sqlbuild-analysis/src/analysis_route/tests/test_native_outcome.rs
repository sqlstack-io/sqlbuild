use crate::analysis_route::main::fast_lineage::fast_lineage;
use crate::analysis_route::main::model_outcome::model_outcome;
use crate::analysis_route::tests::helpers::{column_line, session_relations};
use crate::analysis_route::tests::test_types::{
    CataloguedDialect, FoldingDialect, MarkerLineageTestCase, NativeLineageTestCase,
    NativeOutcomeTestCase, OpenRelationTestCase, PivotlessDialect,
};
use crate::assembly::analysis_session::_helpers::mappings::{ShapeTable, session_schema};
use crate::assembly::analysis_session::models::{
    ModelOutcome, ModelRequest, SessionFunctions, SessionSchema,
};
use crate::assembly::analysis_session::types::Shapes;
use crate::lineage::models::{
    LineageColumn, LineageOutcomeParts, LineageResourceType, LineageSchemaResource,
};
use sqlquack_semantics::lineage::models::{Nullability, NullabilityRule};

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
            nullability_rules: &[],
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
            nullability_rules: &[],
            expected_lines: &["succeeded=true star=true/false findings=[]"],
        },
        NativeOutcomeTestCase {
            description: "SQL the dialect cannot read fails the analysis",
            sql: "SELECT FROM WHERE",
            shape: &[("order_id", "INTEGER", Nullability::NonNull)],
            declared_functions: &[],
            nullability_rules: &[],
            expected_lines: &["succeeded=false star=false/false findings=[]"],
        },
        NativeOutcomeTestCase {
            description: "a function the adapter declares is not unknown",
            sql: "SELECT CURRENT_TIMESTAMP() AS at FROM raw_orders",
            shape: &[("order_id", "INTEGER", Nullability::NonNull)],
            declared_functions: &[("CURRENT_TIMESTAMP", "TIMESTAMPTZ")],
            nullability_rules: &[],
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
            nullability_rules: &[],
            expected_lines: &[
                "at TIMESTAMPTZ unknown",
                "succeeded=true star=false/true findings=[\"B101\"]",
            ],
        },
        NativeOutcomeTestCase {
            description: "a declared function's return type types its call",
            sql: "SELECT shop_day(order_id) AS d FROM raw_orders",
            shape: &[("order_id", "INTEGER", Nullability::NonNull)],
            declared_functions: &[("SHOP_DAY", "DATE")],
            nullability_rules: &[],
            expected_lines: &[
                "d DATE unknown",
                "succeeded=true star=false/true findings=[]",
            ],
        },
        NativeOutcomeTestCase {
            description: "a declared function without a return type is untyped",
            sql: "SELECT shop_day(order_id) AS d FROM raw_orders",
            shape: &[("order_id", "INTEGER", Nullability::NonNull)],
            declared_functions: &[("SHOP_DAY", "")],
            nullability_rules: &[],
            expected_lines: &["d - unknown", "succeeded=true star=false/true findings=[]"],
        },
        NativeOutcomeTestCase {
            description: "an adapter's first-argument rule keeps a non-null argument non-null",
            sql: "SELECT shop_label(status) AS s FROM raw_orders",
            shape: &[("status", "VARCHAR", Nullability::NonNull)],
            declared_functions: &[("SHOP_LABEL", "VARCHAR")],
            nullability_rules: &[("SHOP_LABEL", NullabilityRule::FirstArgument)],
            expected_lines: &[
                "s VARCHAR non_null",
                "succeeded=true star=false/true findings=[]",
            ],
        },
        NativeOutcomeTestCase {
            description: "without the rule the same call's nullability is unknown",
            sql: "SELECT shop_label(status) AS s FROM raw_orders",
            shape: &[("status", "VARCHAR", Nullability::NonNull)],
            declared_functions: &[("SHOP_LABEL", "VARCHAR")],
            nullability_rules: &[],
            expected_lines: &[
                "s VARCHAR unknown",
                "succeeded=true star=false/true findings=[]",
            ],
        },
        NativeOutcomeTestCase {
            description: "a quoted stored name is the quoted column, its doubled quote one quote",
            sql: "SELECT \"Say \"\"hi\"\"\" AS c FROM raw_orders",
            shape: &[("\"Say \"\"hi\"\"\"", "VARCHAR", Nullability::NonNull)],
            declared_functions: &[],
            nullability_rules: &[],
            expected_lines: &[
                "c VARCHAR non_null",
                "succeeded=true star=false/true findings=[]",
            ],
        },
    ];

    for test_case in test_cases {
        let (schema, mut relations): (Shapes, SessionSchema) = session_relations(test_case.shape);
        relations.functions = SessionFunctions {
            return_types: test_case
                .declared_functions
                .iter()
                .filter(|(_, data_type)| !data_type.is_empty())
                .map(|(name, data_type)| ((*name).to_owned(), (*data_type).to_owned()))
                .collect(),
            nullability: test_case
                .nullability_rules
                .iter()
                .map(|(name, rule)| ((*name).to_owned(), *rule))
                .collect(),
        };
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
        let (outcome, failure): (ModelOutcome, Option<String>) = model_outcome(
            &CataloguedDialect,
            &model,
            (schema, &relations),
            (&declared, false),
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
fn given_a_relation_with_declared_columns_only_when_analysing_natively_then_they_type_its_reads() {
    let test_cases = [
        OpenRelationTestCase {
            description: "a declared VARCHAR read in arithmetic is a type error",
            sql: "SELECT status + 1 AS bumped FROM raw_orders",
            declared: &[("status", "VARCHAR"), ("amount", "DOUBLE")],
            expected_lines: &["bumped - unknown", "findings=[\"B212\"]"],
        },
        OpenRelationTestCase {
            description: "a declared column read as it is keeps its declared type",
            sql: "SELECT amount FROM raw_orders",
            declared: &[("status", "VARCHAR"), ("amount", "DOUBLE")],
            expected_lines: &["amount DOUBLE unknown", "findings=[]"],
        },
        OpenRelationTestCase {
            description: "an undeclared column of an open relation is still not refused",
            sql: "SELECT region FROM raw_orders",
            declared: &[("status", "VARCHAR")],
            expected_lines: &["region - unknown", "findings=[]"],
        },
        OpenRelationTestCase {
            description: "with nothing declared, nothing is typed",
            sql: "SELECT status + 1 AS bumped FROM raw_orders",
            declared: &[],
            expected_lines: &["bumped - unknown", "findings=[]"],
        },
    ];
    for test_case in test_cases {
        let schema: Shapes = vec![("raw_orders".to_owned(), Vec::new())];
        let declared: Shapes = vec![(
            "raw_orders".to_owned(),
            test_case
                .declared
                .iter()
                .map(|(name, data_type)| ((*name).to_owned(), (*data_type).to_owned()))
                .collect(),
        )];
        let relations: SessionSchema = session_schema(
            &CataloguedDialect,
            &schema,
            (
                &ShapeTable::from_shapes(&declared),
                &ShapeTable::from_shapes(&Vec::new()),
            ),
            &SessionFunctions::default(),
        );
        let model: ModelRequest = ModelRequest {
            name: "orders".to_owned(),
            query_sql: test_case.sql.to_owned(),
            required_names: vec!["raw_orders".to_owned()],
            ..ModelRequest::default()
        };
        let (outcome, _): (ModelOutcome, Option<String>) = model_outcome(
            &CataloguedDialect,
            &model,
            (schema, &relations),
            (&[], false),
        );
        let mut lines: Vec<String> = outcome
            .analysis
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
        let findings: Vec<&str> = outcome
            .analysis
            .binding_diagnostics
            .iter()
            .map(|row| row.code.as_str())
            .collect();
        lines.push(format!("findings={findings:?}"));
        assert_eq!(lines, test_case.expected_lines, "{}", test_case.description);
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
            description: "repeated calls of one marker read as one relation, so a union stays direct",
            sql: r#"SELECT order_id FROM __ref("orders") UNION ALL SELECT order_id FROM __ref("orders")"#,
            placeholders: &[],
            expected_lines: &[
                "order_id direct high [model:orders.order_id]",
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
        let parts: LineageOutcomeParts<LineageColumn> =
            fast_lineage(&PivotlessDialect, (test_case.sql, &placeholders), &schema).into_parts();
        let mut lines: Vec<String> = parts.columns.iter().map(column_line).collect();
        lines.push(format!("{} star={}", parts.status, parts.has_star));
        lines.extend(parts.detail);
        assert_eq!(lines, test_case.expected_lines, "{}", test_case.description);
    }
}

#[test]
fn given_relation_markers_when_each_dialect_reads_fast_lineage_then_sources_keep_their_resource() {
    let schema: Vec<LineageSchemaResource> = vec![
        LineageSchemaResource {
            resource_type: LineageResourceType::Source,
            name: "orders".to_owned(),
            columns: vec!["order_id".to_owned(), "loaded_at".to_owned()],
        },
        LineageSchemaResource {
            resource_type: LineageResourceType::Model,
            name: "orders".to_owned(),
            columns: vec!["order_id".to_owned(), "amount".to_owned()],
        },
    ];
    let sql: &str = r#"SELECT o.order_id, o.amount, s.loaded_at
FROM __ref("orders") AS o JOIN __source("orders") AS s ON o.order_id = s.order_id"#;
    let expected_lines: &[&str] = &[
        "order_id direct high [model:orders.order_id]",
        "amount direct high [model:orders.amount]",
        "loaded_at direct high [source:orders.loaded_at]",
        "built star=false",
    ];
    let test_cases = [
        MarkerLineageTestCase {
            description: "a lower-case folding dialect matches each marker's name",
            dialect: &PivotlessDialect,
            expected_lines,
        },
        MarkerLineageTestCase {
            description: "a case-preserving dialect matches each marker's name",
            dialect: &CataloguedDialect,
            expected_lines,
        },
        MarkerLineageTestCase {
            description: "an upper-case folding dialect matches each marker's name",
            dialect: &FoldingDialect,
            expected_lines,
        },
    ];

    for test_case in test_cases {
        let parts: LineageOutcomeParts<LineageColumn> =
            fast_lineage(test_case.dialect, (sql, &[]), &schema).into_parts();
        let mut lines: Vec<String> = parts.columns.iter().map(column_line).collect();
        lines.push(format!("{} star={}", parts.status, parts.has_star));
        lines.extend(parts.detail);
        assert_eq!(lines, test_case.expected_lines, "{}", test_case.description);
    }
}
