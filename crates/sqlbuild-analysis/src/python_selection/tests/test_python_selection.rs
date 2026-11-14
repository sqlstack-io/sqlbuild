use crate::python_selection::tests::helpers::{
    excluded_names, expected_names, expected_run, indexed_folder, node_names,
    published_orders_graph, run, run_over,
};
use crate::python_selection::tests::test_types::{
    InventoryTestCase, NodeSelectorTestCase, RunSelectionTestCase,
};

#[test]
fn given_python_node_selectors_when_resolving_then_names_match_python() {
    let test_cases = [
        NodeSelectorTestCase {
            description: "tag",
            select: &["tag:daily"],
            exclude: &[],
            expected_names: Ok(&["export_orders", "prepare_orders"]),
        },
        NodeSelectorTestCase {
            description: "typed selector with upstream",
            select: &["+asset:export_orders"],
            exclude: &[],
            expected_names: Ok(&["export_orders", "prepare_orders"]),
        },
        NodeSelectorTestCase {
            description: "explicit Python path",
            select: &["path:python/tasks"],
            exclude: &[],
            expected_names: Ok(&["prepare_orders"]),
        },
        NodeSelectorTestCase {
            description: "downstream minus checks",
            select: &["prepare_orders+"],
            exclude: &["check:*"],
            expected_names: Ok(&["export_orders", "load_events", "prepare_orders"]),
        },
        NodeSelectorTestCase {
            description: "intersection",
            select: &["task:prepare_orders,tag:orders"],
            exclude: &[],
            expected_names: Ok(&["prepare_orders"]),
        },
        NodeSelectorTestCase {
            description: "path-between selector",
            select: &["a~b"],
            exclude: &[],
            expected_names: Err((
                "S011",
                "path-between selector 'a~b' is not supported for Python nodes yet",
            )),
        },
        NodeSelectorTestCase {
            description: "path without a root",
            select: &["path:other"],
            exclude: &[],
            expected_names: Err((
                "S012",
                "path selectors require an explicit root: use 'models/' or 'python/'",
            )),
        },
        NodeSelectorTestCase {
            description: "unknown tag",
            select: &["tag:missing"],
            exclude: &[],
            expected_names: Err(("S008", "no Python nodes found with tag 'missing'")),
        },
        NodeSelectorTestCase {
            description: "SQL selector kind",
            select: &["seed:countries"],
            exclude: &[],
            expected_names: Err((
                "S010",
                "selector type 'seed' does not map to a Python node type",
            )),
        },
        NodeSelectorTestCase {
            description: "unknown name",
            select: &["missing"],
            exclude: &[],
            expected_names: Err(("S007", "unknown Python node selector 'missing'")),
        },
        NodeSelectorTestCase {
            description: "unit test",
            select: &["test:orders"],
            exclude: &[],
            expected_names: Err((
                "S013",
                "selector 'test:orders' selects a unit test; \
                 only `sqb test` and `sqb build` accept unit-test selectors",
            )),
        },
    ];

    for test_case in test_cases {
        let names = node_names(test_case.select, test_case.exclude);

        assert_eq!(
            names,
            expected_names(test_case.expected_names),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_run_selectors_when_resolving_then_selection_matches_python() {
    let test_cases = [
        RunSelectionTestCase {
            description: "everything without checks, intermediate loader as a source",
            select: &[],
            exclude: &[],
            expected_selection: Ok((
                &[
                    ("model", "orders"),
                    ("source", "load_events"),
                    ("source", "raw_orders"),
                ],
                &[
                    "export_orders",
                    "load_events",
                    "prepare_orders",
                    "raw_orders",
                ],
            )),
        },
        RunSelectionTestCase {
            description: "upstream reaches declared SQL and its terminal loader",
            select: &["+export_orders"],
            exclude: &[],
            expected_selection: Ok((
                &[("model", "orders"), ("source", "raw_orders")],
                &["export_orders", "prepare_orders", "raw_orders"],
            )),
        },
        RunSelectionTestCase {
            description: "downstream reaches Python nodes declaring the SQL",
            select: &["orders+ prepare_orders"],
            exclude: &[],
            expected_selection: Ok((&[("model", "orders")], &["export_orders", "prepare_orders"])),
        },
        RunSelectionTestCase {
            description: "a check whose checked node is selected runs with it",
            select: &["tag:exports"],
            exclude: &[],
            expected_selection: Ok((&[], &["export_orders"])),
        },
        RunSelectionTestCase {
            description: "a check whose checked node is unselected names both",
            select: &["check:check_orders_export"],
            exclude: &[],
            expected_selection: Err((
                "S000",
                "Python check 'check_orders_export' checks asset:export_orders, which the \
                 selection does not include; select it too, or narrow the selector so it no \
                 longer matches the check.",
            )),
        },
        RunSelectionTestCase {
            description: "a node runs without its unselected Python upstream",
            select: &["load_events"],
            exclude: &[],
            expected_selection: Ok((&[("source", "load_events")], &["load_events"])),
        },
        RunSelectionTestCase {
            description: "managed source loader selected as a loader",
            select: &["loader:raw_orders"],
            exclude: &[],
            expected_selection: Err((
                "S007",
                "loader selector 'raw_orders' matches managed source loader 'raw_orders'; \
                 select it as source:raw_orders",
            )),
        },
        RunSelectionTestCase {
            description: "tag on neither side",
            select: &["tag:missing"],
            exclude: &[],
            expected_selection: Err((
                "S000",
                "no SQL resources or Python nodes found with tag 'missing'",
            )),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            run(test_case.select, test_case.exclude),
            expected_run(test_case.expected_selection),
            "{}",
            test_case.description
        );
        assert_eq!(
            run_over(
                &published_orders_graph(),
                test_case.select,
                test_case.exclude
            ),
            expected_run(test_case.expected_selection),
            "{} (published snapshot nodes)",
            test_case.description
        );
    }
}

#[test]
fn given_relative_paths_when_indexing_then_folders_match_python_parent() {
    let test_cases = [
        InventoryTestCase {
            description: "nested file",
            relative_path: "python/tasks/orders.py",
            expected_folder: "python/tasks",
        },
        InventoryTestCase {
            description: "top-level file",
            relative_path: "orders.py",
            expected_folder: ".",
        },
        InventoryTestCase {
            description: "file at the filesystem root",
            relative_path: "/orders.py",
            expected_folder: "/",
        },
    ];

    for test_case in test_cases {
        let folder = indexed_folder(test_case.relative_path);

        assert_eq!(
            folder, test_case.expected_folder,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_check_exclude_tokens_when_resolving_then_unmatched_tokens_are_skipped() {
    let test_cases = [
        NodeSelectorTestCase {
            description: "unknown name, tag and path are skipped",
            select: &[],
            exclude: &["missing tag:missing path:python/none", "tag:exports"],
            expected_names: Ok(&["check_orders_export", "export_orders"]),
        },
        NodeSelectorTestCase {
            description: "other selector failures are raised",
            select: &[],
            exclude: &["path:other"],
            expected_names: Err((
                "S012",
                "path selectors require an explicit root: use 'models/' or 'python/'",
            )),
        },
    ];

    for test_case in test_cases {
        let names = excluded_names(test_case.exclude);

        assert_eq!(
            names,
            expected_names(test_case.expected_names),
            "{} {:?}",
            test_case.description,
            test_case.select
        );
    }
}
