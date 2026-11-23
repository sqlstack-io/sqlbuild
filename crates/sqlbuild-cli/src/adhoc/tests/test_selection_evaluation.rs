//! A selection evaluates as `sqb build` resolves it, refusals point at their token, and the vocabulary counts nodes.

use sqlbuild_orchestration::runs::models::{SelectionField, SelectionProblem, SelectionRefusal};

use crate::adhoc::tests::helpers::{orders_publication, owned_evaluation};
use crate::adhoc::tests::test_types::{
    RefusedPartTestCase, SelectionEvaluationTestCase, VocabularyTestCase,
};

#[test]
fn given_typed_selections_when_evaluated_then_keys_or_located_refusals_match_sqb_build() {
    let test_cases = [
        SelectionEvaluationTestCase {
            description: "a tag selects its SQL resources and Python nodes",
            select: "tag:orders",
            exclude: "",
            expected_evaluation: Ok(&[
                "asset:partner_orders_file",
                "asset:request_carrier_collection",
                "model:orders",
                "model:stg_orders",
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
                    refused: None,
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
                ("sql/models/marts", 3),
                ("sql/models/staging", 1),
                ("python", 4),
                ("python/nodes/fulfillment", 1),
                ("python/nodes/fulfillment/carrier_sync", 1),
                ("python/nodes/loaders", 2),
                ("python/nodes/orders", 1),
                ("python/nodes/orders/partner_export", 1),
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
            expected_entries: &[("asset", 2), ("loader", 2), ("model", 4), ("source", 1)],
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

#[test]
fn given_refused_selectors_when_evaluated_then_the_refused_part_its_type_and_value_are_reported() {
    let test_cases = [
        RefusedPartTestCase {
            description: "a missing tag",
            select: "tag:finanse",
            expected_code: Some("S000"),
            expected_refused: Some(("tag:finanse", 0, "tag", "finanse")),
        },
        RefusedPartTestCase {
            description: "a folder with no models",
            select: "path:sql/models/mart",
            expected_code: Some("S009"),
            expected_refused: Some(("path:sql/models/mart", 0, "path", "sql/models/mart")),
        },
        RefusedPartTestCase {
            description: "an unknown job",
            select: "job:carrier_sink",
            expected_code: Some("S007"),
            expected_refused: Some(("job:carrier_sink", 0, "job", "carrier_sink")),
        },
        RefusedPartTestCase {
            description: "an unknown source",
            select: "source:raw_shipment",
            expected_code: Some("S007"),
            expected_refused: Some(("source:raw_shipment", 0, "source", "raw_shipment")),
        },
        RefusedPartTestCase {
            description: "an unknown seed",
            select: "seed:currencies",
            expected_code: Some("S007"),
            expected_refused: Some(("seed:currencies", 0, "seed", "currencies")),
        },
        RefusedPartTestCase {
            description: "an unknown kind",
            select: "kind:modle",
            expected_code: Some("S007"),
            expected_refused: Some(("kind:modle", 0, "kind", "modle")),
        },
        RefusedPartTestCase {
            description: "an unknown unit test",
            select: "test:orders_unique",
            expected_code: Some("S007"),
            expected_refused: Some(("test:orders_unique", 0, "test", "orders_unique")),
        },
        RefusedPartTestCase {
            description: "an unknown bare name",
            select: "custmers",
            expected_code: Some("S007"),
            expected_refused: Some(("custmers", 0, "name", "custmers")),
        },
        RefusedPartTestCase {
            description: "a name pattern matching nothing",
            select: "missing_*",
            expected_code: Some("S007"),
            expected_refused: Some(("missing_*", 0, "glob", "missing_*")),
        },
        RefusedPartTestCase {
            description: "the second comma part, with its + marker kept in the part but not the value",
            select: "tag:orders,+tag:finanse",
            expected_code: Some("S000"),
            expected_refused: Some(("+tag:finanse", 1, "tag", "finanse")),
        },
        RefusedPartTestCase {
            description: "a selection refused as a whole names no part",
            select: "customers --exclude customers",
            expected_code: None,
            expected_refused: None,
        },
    ];
    let publication = orders_publication();
    for test_case in test_cases {
        let (select, exclude) = test_case
            .select
            .split_once(" --exclude ")
            .unwrap_or((test_case.select, ""));
        let problem = publication.evaluate(select, exclude).err();
        assert_eq!(
            (
                problem.as_ref().and_then(|problem| problem.code.clone()),
                problem.and_then(|problem| problem.refused),
            ),
            (
                test_case.expected_code.map(str::to_owned),
                test_case
                    .expected_refused
                    .map(|(part, index, selector_type, value)| SelectionRefusal {
                        part: part.to_owned(),
                        index,
                        selector_type: selector_type.to_owned(),
                        value: value.to_owned(),
                    }),
            ),
            "{}",
            test_case.description
        );
    }
}
