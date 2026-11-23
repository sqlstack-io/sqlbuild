use crate::scope_index::models::VisibilityReason;
use crate::scope_index::tests::helpers::classified_rows;
use crate::scope_index::tests::test_types::ClassifyTestCase;

#[test]
fn given_consumers_when_classifying_then_records_follow_python_order() {
    let test_cases = [
        ClassifyTestCase {
            description: "global, private, local and inherited positions sort by position",
            path: "sql/models/marts/orders.sql",
            private: &[1],
            grants: &[],
            expected_classified: Some((
                &[
                    (0, "global", None),
                    (1, "private_owner", None),
                    (2, "local_owner", None),
                    (3, "inherited_ancestor", None),
                    (4, "global", None),
                ],
                &[5],
            )),
        },
        ClassifyTestCase {
            description: "grants follow the position's own reason and keep their order",
            path: "sql/models/marts/orders.sql",
            private: &[],
            grants: &[
                (5, VisibilityReason::TestedMacro, 7),
                (0, VisibilityReason::ExpectedModel, 8),
                (5, VisibilityReason::ExpectedModel, 9),
            ],
            expected_classified: Some((
                &[
                    (0, "global", None),
                    (0, "expected_model", Some(8)),
                    (2, "local_owner", None),
                    (3, "inherited_ancestor", None),
                    (4, "global", None),
                    (5, "tested_macro", Some(7)),
                    (5, "expected_model", Some(9)),
                ],
                &[1],
            )),
        },
        ClassifyTestCase {
            description: "a path escaping the project defers to Python",
            path: "../orders.sql",
            private: &[],
            grants: &[],
            expected_classified: None,
        },
    ];

    for test_case in test_cases {
        let classified = classified_rows(&test_case);

        assert_eq!(
            classified
                .as_ref()
                .map(|(visible, inaccessible)| (visible.as_slice(), inaccessible.as_slice())),
            test_case.expected_classified,
            "{}",
            test_case.description
        );
    }
}
