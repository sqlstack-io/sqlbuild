use crate::rule_cache::_helpers::model_rule_key::model_rule_key;
use crate::rule_cache::main::model_rule_keys::model_rule_keys;
use crate::rule_cache::models::ModelKeyInput;
use crate::rule_cache::tests::test_types::ModelRuleKeysTestCase;

#[test]
fn given_models_when_hashing_rule_keys_together_then_each_key_matches_its_single_model_key_in_order()
 {
    let test_cases = [
        ModelRuleKeysTestCase {
            description: "no models give no keys",
            models: vec![],
            expected_key_count: 0,
        },
        ModelRuleKeysTestCase {
            description: "many models keep their order and single-model keys",
            models: (0..64)
                .map(|index| {
                    let sql: &'static str =
                        Box::leak(format!("select {index} as id").into_boxed_str());
                    let contract: Option<&'static str> = (index % 3 == 0).then_some("id:integer");
                    (sql, sql, contract)
                })
                .collect(),
            expected_key_count: 64,
        },
    ];
    let codes: Vec<String> = vec!["SQLCLEAN001".to_owned(), "SQLCLEAN033".to_owned()];

    for test_case in test_cases {
        let inputs: Vec<ModelKeyInput<'_>> = test_case
            .models
            .iter()
            .map(|(authored_sql, query_sql, contract)| ModelKeyInput {
                authored_sql,
                query_sql,
                contract: *contract,
            })
            .collect();
        let expected: Vec<String> = inputs
            .iter()
            .map(|model| model_rule_key("lint-key", &codes, model))
            .collect();

        let keys: Vec<String> = model_rule_keys("lint-key", &codes, &inputs);

        assert_eq!(
            keys.len(),
            test_case.expected_key_count,
            "{}",
            test_case.description
        );
        assert_eq!(keys, expected, "{}", test_case.description);
    }
}
