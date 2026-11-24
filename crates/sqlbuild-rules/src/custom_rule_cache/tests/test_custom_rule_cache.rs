use crate::tests::helpers::install_test_identity;
use std::collections::BTreeMap;

use serde_json::json;

use crate::custom_rule_cache::main::custom_rule_cache_text::custom_rule_cache_text;
use crate::custom_rule_cache::main::lookup_custom_rule_cache::lookup_custom_rule_cache;
use crate::custom_rule_cache::main::parse_custom_rule_cache::parse_custom_rule_cache;
use crate::custom_rule_cache::main::pending_readsets::pending_readsets;
use crate::custom_rule_cache::main::rule_identity::rule_identity;
use crate::custom_rule_cache::main::store_custom_results::store_custom_results;
use crate::custom_rule_cache::models::{CustomEvaluation, CustomRule, RuleIdentityInput};
use crate::custom_rule_cache::tests::helpers::{identity_input, readset, rule};
use crate::custom_rule_cache::tests::test_types::{IdentityTestCase, LookupTestCase};
use crate::custom_rule_cache::types::ReadSet;

#[test]
fn given_rule_identity_inputs_when_one_changes_then_only_relevant_edits_change_the_identity() {
    install_test_identity();
    let test_cases = [
        IdentityTestCase {
            description: "the same inputs give the same identity",
            edit: |_| {},
            expected_changed: false,
        },
        IdentityTestCase {
            description: "an edited rule source changes the identity",
            edit: |input| input.implementation_fingerprint = "source-b".to_owned(),
            expected_changed: true,
        },
        IdentityTestCase {
            description: "edited rules configuration changes the identity",
            edit: |input| input.config_json = "{\"select\":[]}".to_owned(),
            expected_changed: true,
        },
        IdentityTestCase {
            description: "a project-wide subject changes the identity",
            edit: |input| input.subject = Some("project".to_owned()),
            expected_changed: true,
        },
    ];

    for test_case in test_cases {
        let mut edited: RuleIdentityInput = identity_input();
        (test_case.edit)(&mut edited);
        let changed: bool = rule_identity(&edited) != rule_identity(&identity_input());

        assert_eq!(
            changed, test_case.expected_changed,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_stored_results_when_looking_up_then_unchanged_subjects_and_reads_hit() {
    install_test_identity();
    let test_cases = [
        LookupTestCase {
            description: "unchanged reads and subjects hit every subject",
            current_digest: "digest-a",
            model_name: "orders",
            expected_plan: vec![],
            expected_hits: 2,
        },
        LookupTestCase {
            description: "a changed fact digest misses every subject",
            current_digest: "digest-b",
            model_name: "orders",
            expected_plan: vec![("CUSTOM001", None)],
            expected_hits: 0,
        },
        LookupTestCase {
            description: "a renamed model misses only that subject",
            current_digest: "digest-a",
            model_name: "orders_v2",
            expected_plan: vec![("CUSTOM001", Some(vec!["sql/models/orders.sql"]))],
            expected_hits: 1,
        },
    ];

    for test_case in test_cases {
        let stored_rule: CustomRule = rule("orders");
        let evaluations: Vec<CustomEvaluation> = stored_rule
            .subjects
            .iter()
            .map(|subject| CustomEvaluation {
                code: stored_rule.code.clone(),
                subject: subject.key.clone(),
                cacheable: Some((readset(), Some("digest-a".to_owned()))),
                findings: vec![json!({"code": "CUSTOM001", "path": subject.key})],
            })
            .collect();
        let text: String = custom_rule_cache_text(
            &store_custom_results(
                &BTreeMap::new(),
                std::slice::from_ref(&stored_rule),
                &evaluations,
            ),
            &[stored_rule],
        );
        let current: CustomRule = rule(test_case.model_name);
        let stored = parse_custom_rule_cache(&text);
        let digests: BTreeMap<ReadSet, String> =
            pending_readsets(&stored, std::slice::from_ref(&current))
                .into_iter()
                .map(|readset| (readset, test_case.current_digest.to_owned()))
                .collect();
        let lookup = lookup_custom_rule_cache(&stored, &[current], &digests);
        let expected_plan: BTreeMap<String, Option<Vec<String>>> = test_case
            .expected_plan
            .iter()
            .map(|(code, subjects)| {
                let subjects = subjects
                    .as_ref()
                    .map(|items| items.iter().map(|item| (*item).to_owned()).collect());
                ((*code).to_owned(), subjects)
            })
            .collect();

        assert_eq!(lookup.plan, expected_plan, "{}", test_case.description);
        assert_eq!(
            lookup.hit_count, test_case.expected_hits,
            "{}",
            test_case.description
        );
        assert_eq!(
            lookup.counts_by_code.values().copied().collect::<Vec<_>>(),
            vec![(lookup.hit_count, lookup.miss_count)],
            "{}",
            test_case.description
        );
    }
}
