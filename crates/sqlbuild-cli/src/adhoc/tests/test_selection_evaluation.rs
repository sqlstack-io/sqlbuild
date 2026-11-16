//! A selection evaluates as `sqb build` resolves it, refusals point at their token, and the vocabulary counts nodes.

use sqlbuild_orchestration::runs::models::{SelectionField, SelectionProblem};

use crate::adhoc::tests::helpers::{orders_publication, owned_evaluation};
use crate::adhoc::tests::test_types::{SelectionEvaluationTestCase, VocabularyTestCase};

#[test]
fn given_typed_selections_when_evaluated_then_keys_or_located_refusals_match_sqb_build() {
    let test_cases = [
        SelectionEvaluationTestCase {
            description: "a tag selects its SQL resources and Python nodes",
            select: "tag:orders",
            exclude: "",
            expected_evaluation: Ok(&[
                "asset:partner_orders_file",
                "model:orders",
                "model:stg_orders",
                "task:request_carrier_collection",
            ]),
        },
        SelectionEvaluationTestCase {
            description: "+ expands its own part before the kind narrows it",
            select: "kind:model,+orders",
            exclude: "",
            expected_evaluation: Ok(&["model:orders", "model:stg_orders"]),
        },
        SelectionEvaluationTestCase {
            description: "an unknown tag points at its token and suggests the closest tag",
            select: "customers tag:ordrs",
            exclude: "",
            expected_evaluation: Err((
                Some("S000"),
                "no SQL resources or Python nodes found with tag 'ordrs'",
                Some((SelectionField::Select, "tag:ordrs", 10, 19)),
                &["tag:orders"],
            )),
        },
        SelectionEvaluationTestCase {
            description: "an exclude typo is located there; the fix keeps its + and other parts",
            select: "tag:orders",
            exclude: "+kind:modle,tag:orders",
            expected_evaluation: Err((
                Some("S007"),
                "unknown node kind 'modle'",
                Some((SelectionField::Exclude, "+kind:modle,tag:orders", 0, 22)),
                &["+kind:model,tag:orders"],
            )),
        },
        SelectionEvaluationTestCase {
            description: "a bare name typo is fixed in its bare form",
            select: "custmers",
            exclude: "",
            expected_evaluation: Err((
                Some("S007"),
                "unknown selector name 'custmers'",
                Some((SelectionField::Select, "custmers", 0, 8)),
                &["customers"],
            )),
        },
        SelectionEvaluationTestCase {
            description: "a selection with nothing left is refused as a whole, not at a token",
            select: "customers",
            exclude: "customers",
            expected_evaluation: Err((
                None,
                "the selectors match no nodes in the current publication",
                None,
                &[],
            )),
        },
        SelectionEvaluationTestCase {
            description: "positions count UTF-16 code units, as JavaScript indexes the text",
            select: "tag:orders\u{3000}tag:nope",
            exclude: "",
            expected_evaluation: Err((
                Some("S000"),
                "no SQL resources or Python nodes found with tag 'nope'",
                Some((SelectionField::Select, "tag:nope", 11, 19)),
                &[],
            )),
        },
    ];
    let publication = orders_publication();
    for test_case in test_cases {
        let actual = publication
            .evaluate(test_case.select, test_case.exclude)
            .map_err(|problem| {
                Box::new(SelectionProblem {
                    help: None,
                    ..*problem
                })
            });
        assert_eq!(
            actual,
            owned_evaluation(test_case.expected_evaluation),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_publication_when_its_vocabulary_is_read_then_each_value_counts_the_nodes_it_selects() {
    let test_cases = [
        VocabularyTestCase {
            description: "a tag counts its SQL resources and Python nodes",
            entries_of: |vocabulary| &vocabulary.tags,
            expected_entries: &[("orders", 4)],
        },
        VocabularyTestCase {
            description: "a folder counts the nodes below it at any depth",
            entries_of: |vocabulary| &vocabulary.paths,
            expected_entries: &[
                ("models", 4),
                ("models/marts", 3),
                ("models/staging", 1),
                ("python", 4),
                ("python/fulfillment", 1),
                ("python/fulfillment/carrier_sync", 1),
                ("python/loaders", 2),
                ("python/orders", 1),
                ("python/orders/partner_export", 1),
            ],
        },
        VocabularyTestCase {
            description: "a job counts the nodes it selects",
            entries_of: |vocabulary| &vocabulary.jobs,
            expected_entries: &[("carrier_sync", 1)],
        },
        VocabularyTestCase {
            description: "sources and seeds are listed by name",
            entries_of: |vocabulary| &vocabulary.sources,
            expected_entries: &[("raw_shipments", 1)],
        },
        VocabularyTestCase {
            description: "a kind counts its nodes",
            entries_of: |vocabulary| &vocabulary.kinds,
            expected_entries: &[
                ("asset", 1),
                ("loader", 2),
                ("model", 4),
                ("source", 1),
                ("task", 1),
            ],
        },
    ];
    let vocabulary = orders_publication().vocabulary();
    for test_case in test_cases {
        let actual: Vec<(&str, u64)> = (test_case.entries_of)(&vocabulary)
            .iter()
            .map(|entry| (entry.value.as_str(), entry.count))
            .collect();
        assert_eq!(
            actual, test_case.expected_entries,
            "{}",
            test_case.description
        );
    }
}
