use crate::functions::main::function_reference_error::function_reference_error;
use crate::functions::models::KnownResources;
use crate::functions::tests::helpers::names;
use crate::functions::tests::test_types::FunctionReferenceTestCase;

#[test]
fn given_function_references_when_validating_then_first_unresolved_one_is_reported() {
    let (models, seeds, sources) = (names(&["orders"]), names(&["regions"]), names(&["raw"]));
    let (functions, table_functions) = (names(&["score", "open_orders"]), names(&["open_orders"]));
    let known = KnownResources {
        models: &models,
        seeds: &seeds,
        sources: &sources,
        functions: &functions,
        table_functions: &table_functions,
    };
    let test_cases = [
        FunctionReferenceTestCase {
            description: "every reference resolves",
            references: vec![
                ("ref", "orders"),
                ("udf", "score"),
                ("table_fn", "open_orders"),
            ],
            expected_error: None,
        },
        FunctionReferenceTestCase {
            description: "a seed read with __ref names the seed call",
            references: vec![("ref", "regions")],
            expected_error: Some(
                "SQL function file sql/functions/sql/lookup.sql references seed 'regions' with \
                 __ref(...). Use __seed(\"regions\") for seed references; __ref only resolves \
                 models.",
            ),
        },
        FunctionReferenceTestCase {
            description: "a table function called as a scalar UDF",
            references: vec![("ref", "orders"), ("udf", "open_orders")],
            expected_error: Some(
                "SQL function file sql/functions/sql/lookup.sql references table function \
                 'open_orders' with __udf(); use __table_fn()",
            ),
        },
        FunctionReferenceTestCase {
            description: "a scalar function called as a table function",
            references: vec![("table_fn", "score")],
            expected_error: Some(
                "SQL function file sql/functions/sql/lookup.sql references scalar function 'score' \
                 with __table_fn(); use __udf() for scalar UDFs",
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            function_reference_error("sql/functions/sql/lookup.sql", &test_case.references, &known)
                .as_deref(),
            test_case.expected_error,
            "{}",
            test_case.description
        );
    }
}
