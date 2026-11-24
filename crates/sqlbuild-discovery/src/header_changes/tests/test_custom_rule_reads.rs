use crate::header_changes::main::custom_rule_reads::custom_rule_reads;
use crate::header_changes::tests::helpers::keys;
use crate::header_changes::tests::test_types::CustomRuleReadsTestCase;

#[test]
fn given_custom_rule_facts_when_a_header_changes_then_reads_of_changed_keys_are_named() {
    let test_cases = [
        CustomRuleReadsTestCase {
            description: "a fact of the model deciding a changed key is named",
            fact_key: vec!["columns.names", "orders"],
            changed: vec!["columns"],
            expected_reason: Some("a custom rule reads orders's columns"),
        },
        CustomRuleReadsTestCase {
            description: "a model's enum column types are decided by its columns",
            fact_key: vec!["columns.enum_types", "sql/models/orders.sql"],
            changed: vec!["columns"],
            expected_reason: Some("a custom rule reads orders's columns"),
        },
        CustomRuleReadsTestCase {
            description: "a fact of another model is ignored",
            fact_key: vec!["columns.names", "customers"],
            changed: vec!["columns"],
            expected_reason: None,
        },
        CustomRuleReadsTestCase {
            description: "a fact no changed key decides is ignored",
            fact_key: vec!["materialized", "sql/models/orders.sql"],
            changed: vec!["description"],
            expected_reason: None,
        },
    ];
    for test_case in test_cases {
        let facts = vec![(
            test_case
                .fact_key
                .iter()
                .map(|part| (*part).to_owned())
                .collect::<Vec<String>>(),
            "digest".to_owned(),
        )];
        assert_eq!(
            custom_rule_reads(
                &facts,
                ("sql/models/orders.sql", "orders"),
                &keys(&test_case.changed)
            )
            .as_deref(),
            test_case.expected_reason,
            "{}",
            test_case.description
        );
    }
}
