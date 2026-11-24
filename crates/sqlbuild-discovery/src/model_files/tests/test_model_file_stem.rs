use crate::model_files::main::model_file_stem::model_file_stem;
use crate::model_files::tests::test_types::ModelFileStemTestCase;

#[test]
fn given_relative_paths_when_naming_models_then_stems_match_python() {
    let test_cases = [
        ModelFileStemTestCase {
            description: "a final suffix is dropped",
            relative_path: "sql/models/orders.sql",
            expected_stem: "orders",
        },
        ModelFileStemTestCase {
            description: "only the last suffix is dropped",
            relative_path: "sql/models/marts/orders.v2.sql",
            expected_stem: "orders.v2",
        },
        ModelFileStemTestCase {
            description: "a leading dot is not a suffix",
            relative_path: "sql/models/.hidden",
            expected_stem: ".hidden",
        },
        ModelFileStemTestCase {
            description: "a trailing dot is kept",
            relative_path: "orders.",
            expected_stem: "orders.",
        },
        ModelFileStemTestCase {
            description: "a name without a suffix is kept",
            relative_path: "orders",
            expected_stem: "orders",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            model_file_stem(test_case.relative_path),
            test_case.expected_stem,
            "{}",
            test_case.description
        );
    }
}
