use crate::planner_strategy::main::group_replacement_refusal::group_replacement_refusal;
use crate::planner_strategy::tests::test_types::GroupReplacementRefusalTestCase;

#[test]
fn given_replace_key_when_planning_then_only_bigquery_refuses_it() {
    let test_cases = [
        GroupReplacementRefusalTestCase {
            description: "BigQuery's delete_insert matches one source row per target row",
            dialect: Some("bigquery"),
            has_replace_key: true,
            expected_refused: true,
        },
        GroupReplacementRefusalTestCase {
            description: "BigQuery with unique_key keeps its delete_insert",
            dialect: Some("bigquery"),
            has_replace_key: false,
            expected_refused: false,
        },
        GroupReplacementRefusalTestCase {
            description: "Snowflake deletes every row of a key",
            dialect: Some("snowflake"),
            has_replace_key: true,
            expected_refused: false,
        },
        GroupReplacementRefusalTestCase {
            description: "an adapter without SQL analysis",
            dialect: None,
            has_replace_key: true,
            expected_refused: false,
        },
    ];

    for test_case in test_cases {
        let refusal = group_replacement_refusal(
            "forecast_lines",
            ("warehouse", test_case.dialect),
            test_case.has_replace_key,
        );

        assert_eq!(
            refusal.is_some(),
            test_case.expected_refused,
            "{}",
            test_case.description
        );
    }
    assert_eq!(
        group_replacement_refusal("forecast_lines", ("bigquery", Some("bigquery")), true)
            .as_deref(),
        Some(
            "model 'forecast_lines' sets replace_key, but adapter 'bigquery' cannot replace groups of \
             rows; use unique_key with one row per key, or replace groups on any adapter other \
             than BigQuery"
        )
    );
}
