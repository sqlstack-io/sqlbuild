//! Iterative loader DAG ordering and execution eligibility regressions.

use crate::assembly::project::types::ObjectKey;
use crate::planner_scope::main::expand_loaders::expand_loaders;
use crate::planner_scope::main::node_loads::node_loads;
use crate::planner_scope::models::LoaderGraph;
use crate::planner_scope::tests::test_types::{DeepLoaderTestCase, LoaderTestCase};
use std::collections::HashSet;

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
    let graph = LoaderGraph {
        loaders: [
            ("raw_orders".to_owned(), vec!["stage_orders".to_owned()]),
            ("stage_orders".to_owned(), vec!["raw_customers".to_owned()]),
            ("raw_customers".to_owned(), Vec::new()),
            ("raw_returns".to_owned(), Vec::new()),
        ]
        .into_iter()
        .collect(),
        sources: vec![
            ("raw_orders".to_owned(), Some("raw_orders".to_owned())),
            ("raw_customers".to_owned(), Some("raw_customers".to_owned())),
            ("raw_returns".to_owned(), Some("raw_returns".to_owned())),
            ("raw_products".to_owned(), None),
        ],
    };
    let key = |kind: &str, name: &str| (kind.to_owned(), name.to_owned());
    let upstream = vec![
        (
            key("model", "orders"),
            vec![
                key("source", "raw_returns"),
                key("source", "raw_products"),
                key("model", "stg_orders"),
            ],
        ),
        (
            key("model", "stg_orders"),
            vec![key("source", "raw_products")],
        ),
        (key("source", "raw_orders"), Vec::new()),
        (key("source", "raw_customers"), Vec::new()),
        (key("source", "raw_products"), Vec::new()),
    ];
    let loads = node_loads(&graph, &upstream);
    assert_eq!(
        loads
            .iter()
            .map(|(node, writers)| (
                node.1.as_str(),
                writers.iter().map(|w| w.1.as_str()).collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("orders", vec!["raw_returns"]),
            // The chain reaches raw_customers through stage_orders, an intermediate loader no source
            // declares. It is left out on purpose, so a run that loads it still fails in the worker.
            ("raw_orders", vec!["raw_customers"]),
        ]
    );
}
