use std::collections::{HashMap, HashSet};

use crate::planner_identity::_helpers::compose_graph_identity::compose_graph_identity;
use crate::planner_identity::main::classify_selection_staleness::classify_selection_staleness;
use crate::planner_identity::main::expected_graph_identity_hashes::expected_graph_identity_hashes;
use crate::planner_identity::main::function_fingerprint_sql::function_fingerprint_sql;
use crate::planner_identity::main::graph_write_identity_hashes::graph_write_identity_hashes;
use crate::planner_identity::main::missing_upstream_message::missing_upstream_message;
use crate::planner_identity::main::missing_upstreams::missing_upstreams;
use crate::planner_identity::main::stale_upstream_warning_message::stale_upstream_warning_message;
use crate::planner_identity::main::topological_order::topological_order;
use crate::planner_identity::models::{
    AvailableUpstreams, FunctionFingerprintFields, GraphKey, MissingUpstream, StaleWarning,
    StalenessNames,
};
use crate::planner_identity::tests::helpers::{
    chain_head_hash, chain_identity_graph, chain_staleness_graph, cyclic_identity_graph,
    identity_graph, key, ladder_identity_graph, model, names, staleness_graph, texts,
};
use crate::planner_identity::tests::test_types::{
    BuildabilityTestCase, DeepChainTestCase, FunctionSqlTestCase, HashTestCase,
    IdentityGraphSizeTestCase, IdentityGraphTestCase, OrderTestCase, StalenessTestCase,
    WarningTextTestCase,
};
use crate::planner_identity::types::IdentityHashes;
use sqlbuild_core::fingerprint_identity::main::model_local_identity_hash::model_local_identity_hash;
use sqlbuild_core::fingerprint_identity::main::stable_identity_hash::stable_identity_hash;

#[test]
fn given_persisted_identity_inputs_when_hashing_then_matches_pinned_python_hashes() {
    let test_cases = [
        HashTestCase {
            description: "empty text",
            actual: stable_identity_hash(""),
            expected_hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        },
        HashTestCase {
            description: "unicode and carriage returns",
            actual: stable_identity_hash("orders\r\ncafé ☃"),
            expected_hash: "ba7e194792c57a2f2e3aa77441f680ac5823e5a19cbbef0113bd20d701aa4ab2",
        },
        HashTestCase {
            description: "composed with upstreams",
            actual: compose_graph_identity("abc", &["d1", "d2"]),
            expected_hash: "5be4a7558096ea1ae6d8c0f97fac33972a28951ded9ce7e9a2c1898c2b8389cc",
        },
        HashTestCase {
            description: "composed without upstreams",
            actual: compose_graph_identity("abc", &[]),
            expected_hash: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        },
        HashTestCase {
            description: "model local identity",
            actual: model_local_identity_hash("q1", r#"{"config":{}}"#),
            expected_hash: "3f16f63bb7e4c297529a7814cdce6737d9fcc1f3ad695905c58daa2438613c16",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            test_case.actual, test_case.expected_hash,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_identity_graph_when_resolving_then_matches_pinned_python_hashes() {
    let pinned = |hash: &str| hash.to_owned();
    let test_cases = [
        IdentityGraphTestCase {
            description: "acyclic graph with an unknown upstream and a node without a local hash",
            graph: identity_graph(),
            selected: HashSet::from([model("b"), model("c")]),
            base_hashes: vec![(model("a"), "base_a".to_owned())],
            expected_hashes: vec![
                (
                    model("a"),
                    Some(pinned(
                        "b737ca7ee563ae80e457bb3d1dfe64edd2b4c015a8f88b6f87d5c113b68897fd",
                    )),
                ),
                (
                    model("b"),
                    Some(pinned(
                        "b1d269f24b600f5b30f9b68dc34f0305ebf3c23b3657ebb147b6cae7ffaf8282",
                    )),
                ),
                (model("x"), None),
                (
                    model("c"),
                    Some(pinned(
                        "3f7d5648557bef56e83e9b9b9cc1789cf758e32310f142d733b3d57a5caabc15",
                    )),
                ),
                (model("d"), None),
            ],
            expected_write_hashes: vec![
                (model("a"), pinned("base_a")),
                (
                    model("b"),
                    pinned("1780fb66cc0e91d38e4be3c63ffa49e52abaa5365a68aad535ff04218f58ecc9"),
                ),
                (
                    model("c"),
                    pinned("d41afd29f6fdfaa9bc2a55fd9e5f1a75b2e8dbe100f431706a67938e21fd1b86"),
                ),
            ],
        },
        IdentityGraphTestCase {
            description: "cycle broken with the local hash of the node being resolved",
            graph: cyclic_identity_graph(),
            selected: HashSet::from([model("a"), model("b")]),
            base_hashes: Vec::new(),
            expected_hashes: vec![
                (
                    model("a"),
                    Some(pinned(
                        "9c389106b00932fd7ff1b7dd3778747eac217990a9a73b16c80a9f0bbbf9f527",
                    )),
                ),
                (
                    model("b"),
                    Some(pinned(
                        "b8c36ab63f3421b0ce0f3b1b55428334e42f1c1a8db65d10707bdc236a81da76",
                    )),
                ),
            ],
            expected_write_hashes: vec![
                (
                    model("b"),
                    pinned("b8c36ab63f3421b0ce0f3b1b55428334e42f1c1a8db65d10707bdc236a81da76"),
                ),
                (
                    model("a"),
                    pinned("9c389106b00932fd7ff1b7dd3778747eac217990a9a73b16c80a9f0bbbf9f527"),
                ),
            ],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            (
                expected_graph_identity_hashes(&test_case.graph),
                graph_write_identity_hashes(
                    &test_case.graph,
                    &test_case.selected,
                    test_case.base_hashes
                ),
            ),
            (test_case.expected_hashes, test_case.expected_write_hashes),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_dense_ladder_when_resolving_write_hashes_then_resolves_each_node_once() {
    let test_cases = [IdentityGraphSizeTestCase {
        description: "four hundred fully selected layers",
        layer_count: 400,
        expected_hash_count: 800,
    }];
    for test_case in test_cases {
        let graph = ladder_identity_graph(test_case.layer_count);
        let selected: HashSet<GraphKey> = graph.nodes.keys().cloned().collect();
        assert_eq!(
            graph_write_identity_hashes(&graph, &selected, Vec::new()).len(),
            test_case.expected_hash_count,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_deep_chain_when_walking_graph_then_does_not_overflow_the_stack() {
    let test_cases = [DeepChainTestCase {
        description: "one hundred thousand chained models",
        depth: 100_000,
        expected_hash_count: 100_000,
        expected_trigger_count: 99_999,
    }];
    for test_case in test_cases {
        let graph = chain_identity_graph(test_case.depth);
        let head: GraphKey = model(&format!("m{}", test_case.depth - 1));
        let expected: IdentityHashes = expected_graph_identity_hashes(&graph);
        assert_eq!(
            expected.len(),
            test_case.expected_hash_count,
            "{}",
            test_case.description
        );
        assert_eq!(
            expected.last().map(|(_, hash)| hash.clone()),
            Some(chain_head_hash(test_case.depth)),
            "{}",
            test_case.description
        );
        let selected: HashSet<GraphKey> = graph.nodes.keys().cloned().collect();
        let written: Vec<(GraphKey, String)> =
            graph_write_identity_hashes(&graph, &selected, Vec::new());
        assert_eq!(
            written.len(),
            test_case.expected_hash_count,
            "{}",
            test_case.description
        );
        assert_eq!(
            written
                .iter()
                .find(|(key, _)| *key == head)
                .map(|(_, hash)| hash.clone()),
            chain_head_hash(test_case.depth),
            "{}",
            test_case.description
        );
        let warnings: Vec<StaleWarning> =
            classify_selection_staleness(&chain_staleness_graph(test_case.depth));
        assert_eq!(
            warnings
                .iter()
                .map(|warning| warning.trigger_names.len())
                .sum::<usize>(),
            test_case.expected_trigger_count,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_function_fields_when_rendering_then_writes_one_field_per_line() {
    let test_cases = [FunctionSqlTestCase {
        description: "python function with packages and an unnamed argument",
        fields: FunctionFingerprintFields {
            language: "python".to_owned(),
            arguments: vec![
                ("x".to_owned(), "int".to_owned()),
                (String::new(), "text".to_owned()),
            ],
            returns: "table".to_owned(),
            return_columns: vec![("total".to_owned(), "double".to_owned())],
            runtime_version: "3.12".to_owned(),
            entry_point: String::new(),
            packages: texts(&["numpy", "pandas"]),
            body_sql: "return x\r\n".to_owned(),
        },
        expected_sql: "language=python\narguments=x:int,:text\nreturns=table\nreturn_columns=total:double\n\
             runtime_version=3.12\nentry_point=\npackages=numpy,pandas\nbody=\nreturn x\r\n",
    }];
    for test_case in test_cases {
        assert_eq!(
            function_fingerprint_sql(&test_case.fields),
            test_case.expected_sql,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_selection_when_classifying_staleness_then_names_changed_upstreams_outside_run() {
    let mut changed_outside = staleness_graph(
        &[
            (model("report"), &[model("orders"), key("seed", "regions")]),
            (model("orders"), &[model("raw_orders")]),
        ],
        &["report"],
    );
    changed_outside.changed = StalenessNames {
        models: names(&["raw_orders"]),
        seeds: names(&["regions"]),
        sources: HashSet::new(),
    };
    let mut run_parent = staleness_graph(
        &[
            (model("report"), &[model("orders")]),
            (model("orders"), &[model("raw_orders")]),
        ],
        &["report"],
    );
    run_parent.run.models = names(&["raw_orders", "report"]);
    run_parent.changed.models = names(&["raw_orders"]);
    let mut source_in_run = staleness_graph(
        &[(model("report"), &[key("source", "events")])],
        &["report"],
    );
    source_in_run.changed.sources = names(&["events"]);
    source_in_run.run.sources = names(&["events"]);
    let test_cases = [
        StalenessTestCase {
            description: "changed model and seed outside the run set",
            graph: changed_outside,
            expected_warnings: vec![StaleWarning {
                model_name: "report".to_owned(),
                trigger_names: texts(&["orders", "raw_orders", "regions"]),
            }],
        },
        StalenessTestCase {
            description: "changed parent runs but the middle model does not",
            graph: run_parent,
            expected_warnings: vec![StaleWarning {
                model_name: "report".to_owned(),
                trigger_names: texts(&["orders"]),
            }],
        },
        StalenessTestCase {
            description: "changed source in the run set",
            graph: source_in_run,
            expected_warnings: Vec::new(),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            classify_selection_staleness(&test_case.graph),
            test_case.expected_warnings,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_trigger_names_when_formatting_then_caps_the_list() {
    let test_cases = [
        WarningTextTestCase {
            description: "one trigger",
            trigger_names: texts(&["orders"]),
            expected_message: "selected model 'report' will build on 1 stale upstream(s) not selected for rebuild:\n    - orders\n    rebuild the closure to refresh them: --select +report",
        },
        WarningTextTestCase {
            description: "more than the display limit",
            trigger_names: texts(&["a", "b", "c", "d", "e", "f", "g"]),
            expected_message: "selected model 'report' will build on 7 stale upstream(s) not selected for rebuild:\n    - a\n    - b\n    - c\n    - d\n    - e\n    +2 more\n    rebuild the closure to refresh them: --select +report",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            stale_upstream_warning_message(
                ("selected model", "report"),
                "upstream(s)",
                &test_case.trigger_names
            ),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_upstream_edges_when_ordering_then_orders_stably_or_reports_cycles() {
    let test_cases = [
        OrderTestCase {
            description: "smallest ready key first",
            upstream: vec![
                (model("report"), vec![model("orders"), key("seed", "regions")]),
                (model("orders"), vec![key("source", "raw")]),
            ],
            origins: HashMap::new(),
            expected_order: Ok(vec![
                key("seed", "regions"),
                key("source", "raw"),
                model("orders"),
                model("report"),
            ]),
        },
        OrderTestCase {
            description: "cycle through an injected edge",
            upstream: vec![
                (model("a"), vec![key("sql_test", "t")]),
                (key("sql_test", "t"), vec![model("a")]),
                (model("z"), vec![]),
            ],
            origins: HashMap::from([(
                (model("a"), key("sql_test", "t")),
                "SQL test 't' runs before 'a'".to_owned(),
            )]),
            expected_order: Err("Dependency cycle detected involving: model:a, sql_test:t (via SQL test 't' runs before 'a')".to_owned()),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            topological_order(&test_case.upstream, &test_case.origins),
            test_case.expected_order,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_selected_keys_when_checking_buildability_then_reports_missing_upstreams() {
    let test_cases = [
        BuildabilityTestCase {
            description: "lineage upstream missing for two dependents",
            selected: vec![model("b"), model("a")],
            upstream: HashMap::from([
                (model("a"), vec![model("raw"), key("source", "events")]),
                (model("b"), vec![model("raw"), model("built"), model("a")]),
            ]),
            available: AvailableUpstreams {
                existing_relations: names(&["built"]),
                ..AvailableUpstreams::default()
            },
            origins: HashMap::new(),
            expected_missing: vec![MissingUpstream {
                key: model("raw"),
                required_by: vec![model("a"), model("b")],
            }],
            expected_message: "cannot build selected scope: 1 missing upstream dependencies (raw)",
        },
        BuildabilityTestCase {
            description: "only an audit read requires the upstream",
            selected: vec![model("a")],
            upstream: HashMap::from([(model("a"), vec![model("lookup")])]),
            available: AvailableUpstreams::default(),
            origins: HashMap::from([(
                (model("a"), model("lookup")),
                "audit 'check' on 'a' reads 'lookup'".to_owned(),
            )]),
            expected_missing: vec![MissingUpstream {
                key: model("lookup"),
                required_by: vec![model("a")],
            }],
            expected_message: "cannot build selected scope: audit 'check' on 'a' reads 'lookup', which is not selected and does not exist in the warehouse; select it or build it first",
        },
    ];
    for test_case in test_cases {
        let missing = missing_upstreams(
            &test_case.selected,
            &test_case.upstream,
            &test_case.available,
        );
        assert_eq!(
            (
                missing_upstream_message(&missing, &test_case.origins),
                missing
            ),
            (
                test_case.expected_message.to_owned(),
                test_case.expected_missing
            ),
            "{}",
            test_case.description
        );
    }
}
