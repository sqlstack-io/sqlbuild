//! Iterative loader DAG ordering and execution eligibility regressions.

use crate::assembly::project::types::ObjectKey;
use crate::planner_scope::main::expand_loaders::expand_loaders;
use crate::planner_scope::main::node_loads::node_loads;
use crate::planner_scope::models::LoaderGraph;
use crate::planner_scope::tests::test_types::{
    DeepLoaderTestCase, LoaderTestCase, NodeLoadsTestCase,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[test]
fn given_shared_loader_dependencies_when_expanding_then_preserves_depth_first_edges_and_breadth_first_names()
 {
    let test_cases = [
        LoaderTestCase {
            description: "read only upstream",
            executable: vec![],
            expected_selected: vec!["raw_orders"],
        },
        LoaderTestCase {
            description: "execute upstream",
            executable: vec!["raw_orders"],
            expected_selected: vec!["raw_orders", "first", "shared", "last", "leaf"],
        },
    ];
    for test_case in test_cases {
        let graph = LoaderGraph {
            loaders: [
                (
                    "terminal".to_owned(),
                    vec!["first".to_owned(), "shared".to_owned()],
                ),
                (
                    "first".to_owned(),
                    vec!["shared".to_owned(), "last".to_owned()],
                ),
                ("shared".to_owned(), vec!["leaf".to_owned()]),
                ("last".to_owned(), vec!["leaf".to_owned()]),
                ("leaf".to_owned(), Vec::new()),
            ]
            .into_iter()
            .collect(),
            sources: vec![("raw_orders".to_owned(), Some("terminal".to_owned()))],
        };
        let root: ObjectKey = ("source".to_owned(), "raw_orders".to_owned());
        let executable: HashSet<ObjectKey> = test_case
            .executable
            .iter()
            .map(|name| ("source".to_owned(), (*name).to_owned()))
            .collect();
        let (selected, edges) = expand_loaders(&graph, vec![root], Vec::new(), &executable);
        assert_eq!(
            selected
                .iter()
                .map(|key| key.1.as_str())
                .collect::<Vec<_>>(),
            test_case.expected_selected,
            "{}",
            test_case.description
        );
        assert_eq!(
            edges
                .iter()
                .map(|(key, _)| key.1.as_str())
                .collect::<Vec<_>>(),
            vec!["raw_orders", "first", "shared", "leaf", "last"],
            "{}",
            test_case.description
        );
        assert_eq!(
            graph.upstream(&graph.loaders["terminal"]),
            vec!["first", "shared", "last", "leaf"],
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_deep_loader_chain_when_expanding_then_uses_iterative_traversal() {
    let test_cases = [DeepLoaderTestCase {
        description: "deep validated chain",
        depth: 2048,
        expected_count: 2048,
    }];
    for test_case in test_cases {
        let mut graph = LoaderGraph {
            loaders: (0..test_case.depth - 1)
                .map(|index| {
                    (
                        format!("loader_{index}"),
                        vec![format!("loader_{}", index + 1)],
                    )
                })
                .collect(),
            ..LoaderGraph::default()
        };
        graph
            .loaders
            .insert(format!("loader_{}", test_case.depth - 1), Vec::new());
        graph
            .sources
            .push(("raw_orders".to_owned(), Some("loader_0".to_owned())));
        let root: ObjectKey = ("source".to_owned(), "raw_orders".to_owned());
        let (selected, edges) = expand_loaders(
            &graph,
            vec![root.clone()],
            Vec::new(),
            &HashSet::from([root]),
        );
        assert_eq!(
            selected.len(),
            test_case.expected_count,
            "{}",
            test_case.description
        );
        assert_eq!(
            edges.len(),
            test_case.expected_count,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_loader_chains_and_unmanaged_reads_when_listing_node_loads_then_only_declared_source_writers_appear()
 {
    let test_cases = [
        NodeLoadsTestCase {
            description: "a hook-read managed source is listed and an unmanaged read is not",
            loaders: vec![("raw_returns", vec![])],
            sources: vec![("raw_returns", Some("raw_returns")), ("raw_products", None)],
            upstream: vec![
                (
                    "model:orders",
                    vec![
                        "source:raw_returns",
                        "source:raw_products",
                        "model:stg_orders",
                    ],
                ),
                ("model:stg_orders", vec!["source:raw_products"]),
                ("source:raw_products", vec![]),
            ],
            expected_loads: vec![("model:orders", vec!["source:raw_returns"])],
        },
        NodeLoadsTestCase {
            // stage_orders has no declaring source; the snapshot publishes it as source:stage_orders.
            description: "a chain keeps an intermediate loader and the declared loaders behind it",
            loaders: vec![
                ("raw_orders", vec!["stage_orders"]),
                ("stage_orders", vec!["raw_customers"]),
                ("raw_customers", vec![]),
            ],
            sources: vec![
                ("raw_orders", Some("raw_orders")),
                ("raw_customers", Some("raw_customers")),
            ],
            upstream: vec![
                ("source:raw_orders", vec![]),
                ("source:raw_customers", vec![]),
            ],
            expected_loads: vec![(
                "source:raw_orders",
                vec!["source:stage_orders", "source:raw_customers"],
            )],
        },
    ];
    let key = |text: &str| {
        let (kind, name) = text.split_once(':').expect("kind:name");
        (kind.to_owned(), name.to_owned())
    };
    for test_case in test_cases {
        let graph = LoaderGraph {
            loaders: test_case
                .loaders
                .iter()
                .map(|(name, deps)| {
                    (
                        (*name).to_owned(),
                        deps.iter().map(|dep| (*dep).to_owned()).collect(),
                    )
                })
                .collect(),
            sources: test_case
                .sources
                .iter()
                .map(|(name, loader)| ((*name).to_owned(), loader.map(str::to_owned)))
                .collect(),
        };
        let upstream: Vec<(ObjectKey, Vec<ObjectKey>)> = test_case
            .upstream
            .iter()
            .map(|(node, deps)| (key(node), deps.iter().map(|dep| key(dep)).collect()))
            .collect();
        assert_eq!(
            node_loads(&graph, &upstream),
            test_case
                .expected_loads
                .iter()
                .map(|(node, writers)| (
                    key(node),
                    writers.iter().map(|writer| key(writer)).collect()
                ))
                .collect::<BTreeMap<ObjectKey, BTreeSet<ObjectKey>>>(),
            "{}",
            test_case.description
        );
    }
}
