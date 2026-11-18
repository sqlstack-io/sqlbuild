use crate::assembly::maintained_checks::main::never_refreshed_maintained::never_refreshed_maintained;
use crate::assembly::maintained_checks::tests::helpers::maintained;
use crate::assembly::maintained_checks::tests::test_types::NeverRefreshedTestCase;

#[test]
fn given_maintained_chains_when_checking_then_unreached_models_without_refresh_are_named() {
    let test_cases = [
        NeverRefreshedTestCase {
            description: "an upstream model refreshed for its reader",
            models: vec![
                maintained("clean_orders", false, &["orders"]),
                maintained("daily_totals", true, &["clean_orders"]),
            ],
            expected_name: None,
        },
        NeverRefreshedTestCase {
            description: "the last model of a chain without refresh",
            models: vec![
                maintained("clean_orders", false, &["orders"]),
                maintained("daily_totals", false, &["clean_orders"]),
            ],
            expected_name: Some("daily_totals"),
        },
        NeverRefreshedTestCase {
            description: "a model read only by a table, which never asks for a refresh",
            models: vec![maintained("clean_orders", false, &["orders"])],
            expected_name: Some("clean_orders"),
        },
        NeverRefreshedTestCase {
            description: "two unread models report the first by name",
            models: vec![
                maintained("b_totals", false, &["orders"]),
                maintained("a_totals", false, &["orders"]),
            ],
            expected_name: Some("a_totals"),
        },
        NeverRefreshedTestCase {
            description: "every model sets its own refresh",
            models: vec![maintained("daily_totals", true, &[])],
            expected_name: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            never_refreshed_maintained(&test_case.models),
            test_case.expected_name,
            "{}",
            test_case.description
        );
    }
}
