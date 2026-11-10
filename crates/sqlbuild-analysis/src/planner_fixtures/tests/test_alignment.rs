use crate::planner_fixtures::main::align_columns::align_columns;
use crate::planner_fixtures::models::{AlignmentInput, RequiredColumn};
use crate::planner_fixtures::tests::helpers::column;
use crate::planner_fixtures::tests::test_types::AlignmentTestCase;

#[test]
fn given_partial_fixtures_when_aligning_then_preserves_schema_and_diagnostic_order() {
    let test_cases = [
        AlignmentTestCase {
            description: "nullable completion",
            supplied: &["order_id"],
            required: &["status", "details"],
            star: false,
            typed_nulls: false,
            expected_completed: Some(vec![1, 2]),
            expected_diagnostics: &[],
        },
        AlignmentTestCase {
            description: "duplicate requirement completes once",
            supplied: &["order_id"],
            required: &["status", "status"],
            star: false,
            typed_nulls: false,
            expected_completed: Some(vec![1]),
            expected_diagnostics: &[],
        },
        AlignmentTestCase {
            description: "unknown and required diagnostics",
            supplied: &["details"],
            required: &["unknown", "order_id"],
            star: false,
            typed_nulls: false,
            expected_completed: None,
            expected_diagnostics: &[
                "mock source 'orders' is missing required columns: unknown; provide explicit values because their types are not authoritative (read by: orders)",
                "mock source 'orders' must provide required non-nullable columns: order_id",
            ],
        },
        AlignmentTestCase {
            description: "schema prefix order",
            supplied: &["status", "order_id"],
            required: &["details"],
            star: true,
            typed_nulls: false,
            expected_completed: None,
            expected_diagnostics: &[
                "mock source 'orders' supplies columns out of authoritative order for a SELECT * closure; provide the leading relation columns in schema order",
            ],
        },
        AlignmentTestCase {
            description: "complete fixture",
            supplied: &["order_id", "status", "details"],
            required: &["status"],
            star: true,
            typed_nulls: false,
            expected_completed: None,
            expected_diagnostics: &[],
        },
        AlignmentTestCase {
            description: "a typed null fixture appends nothing, so its own column order stands",
            supplied: &["status", "order_id", "details"],
            required: &["status"],
            star: true,
            typed_nulls: true,
            expected_completed: Some(vec![]),
            expected_diagnostics: &[],
        },
        AlignmentTestCase {
            description: "typed null fixture",
            supplied: &["order_id", "status", "details"],
            required: &["status"],
            star: true,
            typed_nulls: true,
            expected_completed: Some(vec![]),
            expected_diagnostics: &[],
        },
    ];
    let metadata = vec![
        column("order_id", "INTEGER", false),
        column("status", "VARCHAR", true),
        column("details", "STRUCT(value INTEGER)", true),
    ];
    for test_case in test_cases {
        let supplied: Vec<_> = test_case
            .supplied
            .iter()
            .map(|name| column(name, "UNKNOWN", true))
            .collect();
        let required: Vec<_> = test_case
            .required
            .iter()
            .map(|name| RequiredColumn {
                name: (*name).into(),
                folded_name: (*name).into(),
                readers: vec!["orders".into()],
            })
            .collect();
        let actual = align_columns(
            "mock source 'orders'",
            AlignmentInput {
                supplied: &supplied,
                metadata: &metadata,
                required: &required,
                star: test_case.star,
                typed_nulls: test_case.typed_nulls,
            },
        );
        assert_eq!(
            actual.completed, test_case.expected_completed,
            "{}",
            test_case.description
        );
        assert_eq!(
            actual.diagnostics, test_case.expected_diagnostics,
            "{}",
            test_case.description
        );
    }
}
