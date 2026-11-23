use crate::dbt_graph::errors::DbtSelectionError;
use crate::dbt_graph::main::dbt_downstream_sqlbuild_models::dbt_downstream_sqlbuild_models;
use crate::dbt_graph::main::dbt_fqn_selector_term::dbt_fqn_selector_term;
use crate::dbt_graph::models::DbtSelection;
use crate::dbt_graph::tests::helpers::{graph, select, selection, stable_edges};
use crate::dbt_graph::tests::test_types::{GraphTestCase, SelectionTestCase, TextTestCase};

#[test]
fn given_manifest_and_project_when_building_graph_then_sorts_edges_in_node_order() {
    let test_cases = [
        GraphTestCase {
            description: "upstream edges keep duplicates and drop unknown dependencies",
            edges: stable_edges(&graph().upstream),
            expected_edges: vec![
                (
                    "dbt:model:model.a.stg",
                    vec!["dbt:source:seed.a.codes", "dbt:source:source.a.raw.orders"],
                ),
                ("dbt:source:source.a.raw.orders", vec![]),
                ("dbt:source:seed.a.codes", vec![]),
                (
                    "dbt:model:model.a.int",
                    vec!["dbt:model:model.a.stg", "dbt:model:model.a.stg"],
                ),
                ("dbt:model:model.a.dup", vec![]),
                ("dbt:model:model.b.dup", vec![]),
                ("sqb:model:fact", vec!["dbt:model:model.a.int"]),
                ("sqb:model:mart", vec!["sqb:model:fact"]),
                ("sqb:model:local", vec![]),
            ],
        },
        GraphTestCase {
            description: "downstream edges invert upstream edges",
            edges: stable_edges(&graph().downstream),
            expected_edges: vec![
                (
                    "dbt:model:model.a.stg",
                    vec!["dbt:model:model.a.int", "dbt:model:model.a.int"],
                ),
                (
                    "dbt:source:source.a.raw.orders",
                    vec!["dbt:model:model.a.stg"],
                ),
                ("dbt:source:seed.a.codes", vec!["dbt:model:model.a.stg"]),
                ("dbt:model:model.a.int", vec!["sqb:model:fact"]),
                ("dbt:model:model.a.dup", vec![]),
                ("dbt:model:model.b.dup", vec![]),
                ("sqb:model:fact", vec!["sqb:model:mart"]),
                ("sqb:model:mart", vec![]),
                ("sqb:model:local", vec![]),
            ],
        },
    ];

    for test_case in test_cases {
        let edges: Vec<(&str, Vec<&str>)> = test_case
            .edges
            .iter()
            .map(|(key, targets)| (key.as_str(), targets.iter().map(String::as_str).collect()))
            .collect();
        assert_eq!(edges, test_case.expected_edges, "{}", test_case.description);
    }
}

#[test]
fn given_selectors_when_resolving_then_returns_models_dbt_nodes_and_anchors() {
    let test_cases = [
        SelectionTestCase {
            description: "upstream expansion requires dbt models and sources",
            select: vec!["+fact"],
            exclude: vec![],
            anchors: vec![],
            expected_selection: Ok(selection(
                &["fact"],
                &[
                    "model.a.int",
                    "model.a.stg",
                    "seed.a.codes",
                    "source.a.raw.orders",
                ],
            )),
        },
        SelectionTestCase {
            description: "downstream expansion then exclusion",
            select: vec!["fact+"],
            exclude: vec!["mart+"],
            anchors: vec![],
            expected_selection: Ok(selection(&["fact"], &[])),
        },
        SelectionTestCase {
            description: "dbt to SQLBuild path with downstream expansion",
            select: vec!["stg ~ fact+"],
            exclude: vec![],
            anchors: vec![],
            expected_selection: Ok(selection(
                &["fact", "mart"],
                &["model.a.int", "model.a.stg"],
            )),
        },
        SelectionTestCase {
            description: "a path with no route selects nothing",
            select: vec!["local~fact"],
            exclude: vec![],
            anchors: vec![],
            expected_selection: Ok(selection(&[], &[])),
        },
        SelectionTestCase {
            description: "trailing-plus dbt anchors record every term in order",
            select: vec!["tag:x+", "state:modified", "tag:x+", "path:models\\marts"],
            exclude: vec![],
            anchors: vec![("tag:x+", vec!["model.a.stg"])],
            expected_selection: Ok(DbtSelection {
                dbt_anchor_terms: vec!["tag:x+".to_owned(), "tag:x+".to_owned()],
                dbt_anchor_unique_ids_by_term: vec![(
                    "tag:x+".to_owned(),
                    vec!["model.a.stg".to_owned()],
                )],
                path_translations: vec![(
                    "path:models\\marts".to_owned(),
                    "path:models/marts".to_owned(),
                )],
                ..selection(&["fact", "mart"], &["model.a.int"])
            }),
        },
        SelectionTestCase {
            description: "an ambiguous dbt endpoint is a path selector error",
            select: vec!["dup~fact"],
            exclude: vec![],
            anchors: vec![],
            expected_selection: Err(DbtSelectionError::PathSelector(
                "path selector 'dup~fact' endpoint 'dup' is ambiguous across dbt models".to_owned(),
            )),
        },
        SelectionTestCase {
            description: "an unknown endpoint is a path selector error",
            select: vec!["fact~nope"],
            exclude: vec![],
            anchors: vec![],
            expected_selection: Err(DbtSelectionError::PathSelector(
                "path selector 'fact~nope' endpoint 'nope' is not a known SQLBuild or dbt model"
                    .to_owned(),
            )),
        },
        SelectionTestCase {
            description: "a missing endpoint name is a path selector error",
            select: vec!["fact~ "],
            exclude: vec![],
            anchors: vec![],
            expected_selection: Err(DbtSelectionError::PathSelector(
                "path selector 'fact~ ' requires names on both sides of '~'".to_owned(),
            )),
        },
        SelectionTestCase {
            description: "resolver failures pass through",
            select: vec!["++"],
            exclude: vec![],
            anchors: vec![],
            expected_selection: Err(DbtSelectionError::Resolver(
                "selector '++' has no name".to_owned(),
            )),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            select(&test_case.select, &test_case.exclude, &test_case.anchors),
            test_case.expected_selection,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_dbt_nodes_when_rendering_terms_then_returns_names() {
    let test_cases = [
        TextTestCase {
            description: "SQLBuild models downstream of dbt models",
            rendered: dbt_downstream_sqlbuild_models(
                &graph(),
                &["model.a.stg".to_owned(), "model.unknown".to_owned()],
            ),
            expected_text: vec!["fact", "mart"],
        },
        TextTestCase {
            description: "FQN selector terms and their fallback",
            rendered: vec![
                dbt_fqn_selector_term(&["a".to_owned(), "orders".to_owned()], "orders"),
                dbt_fqn_selector_term(&[], "orders"),
            ],
            expected_text: vec!["fqn:a.orders", "orders"],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            test_case.rendered, test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}
