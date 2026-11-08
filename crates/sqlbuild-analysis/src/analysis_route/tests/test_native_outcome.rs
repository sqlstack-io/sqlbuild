use crate::analysis_route::tests::test_types::{CataloguedDialect, NativeOutcomeTestCase};
use crate::assembly::analysis_session::_helpers::mappings::ShapeTable;
use crate::assembly::analysis_session::_helpers::native_outcome::native_outcome;
use crate::assembly::analysis_session::models::{ModelOutcome, ModelRequest};
use crate::assembly::analysis_session::types::Shapes;

#[test]
fn given_session_shapes_when_a_native_route_analyses_a_model_then_its_facts_follow_them() {
    let test_cases = [
        NativeOutcomeTestCase {
            description: "a closed source expands a star with its types and known nullability",
            sql: "SELECT * FROM raw_orders",
            shape: &[("order_id", "INTEGER"), ("status", "VARCHAR")],
            expected: &[
                "order_id INTEGER non_null",
                "status VARCHAR nullable",
                "succeeded=true star=true/true",
            ],
        },
        NativeOutcomeTestCase {
            description: "a relation the session has no shape for is unknown, not empty",
            sql: "SELECT * FROM raw_orders",
            shape: &[],
            expected: &["succeeded=true star=true/false"],
        },
        NativeOutcomeTestCase {
            description: "SQL the dialect cannot read fails the analysis",
            sql: "SELECT FROM WHERE",
            shape: &[("order_id", "INTEGER")],
            expected: &["succeeded=false star=false/false"],
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
        let (outcome, failure): (ModelOutcome, Option<String>) =
            native_outcome(&CataloguedDialect, &model, (schema, &nullability));
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
        lines.push(format!(
            "succeeded={} star={}/{}",
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
