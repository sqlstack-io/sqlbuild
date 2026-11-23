use crate::node_files::main::node_file_problems::node_file_problems;
use crate::node_files::tests::helpers::module;
use crate::node_files::tests::test_types::NodeFilesTestCase;

#[test]
fn given_node_modules_when_checking_file_names_then_each_declaration_sits_in_its_kind_file() {
    let test_cases = [
        NodeFilesTestCase {
            description: "kind-named files and checks beside a node",
            modules: vec![
                module(
                    "python/nodes/sales/orders/asset.py",
                    &[("orders", "asset")],
                    &[],
                ),
                module(
                    "python/nodes/sales/orders/checks.py",
                    &[("orders_ok", "check")],
                    &[],
                ),
                module(
                    "python/nodes/raw/load/loaders.py",
                    &[],
                    &[("make", &["loader"])],
                ),
                module(
                    "python/nodes/mix/build/factory.py",
                    &[],
                    &[("make", &["asset", "check"])],
                ),
                module("python/nodes/sales/helpers.py", &[], &[]),
            ],
            expected_problem: None,
        },
        NodeFilesTestCase {
            description: "an asset in node.py",
            modules: vec![module(
                "python/nodes/sales/orders/node.py",
                &[("orders", "asset")],
                &[],
            )],
            expected_problem: Some(
                "@asset 'orders' is declared in python/nodes/sales/orders/node.py; an @asset is \
                 declared in asset.py",
            ),
        },
        NodeFilesTestCase {
            description: "an asset in loader.py",
            modules: vec![module(
                "python/nodes/raw/x/loader.py",
                &[("x", "asset")],
                &[],
            )],
            expected_problem: Some(
                "@asset 'x' is declared in python/nodes/raw/x/loader.py; an @asset is declared in \
                 asset.py",
            ),
        },
        NodeFilesTestCase {
            description: "a loaders.py factory returning an asset",
            modules: vec![module(
                "python/nodes/raw/x/loaders.py",
                &[],
                &[("make", &["asset"])],
            )],
            expected_problem: Some(
                "@factory 'make' in python/nodes/raw/x/loaders.py makes only assets; a factory \
                 that makes only assets is declared in assets.py",
            ),
        },
        NodeFilesTestCase {
            description: "a factory.py making one kind",
            modules: vec![module(
                "python/nodes/raw/x/factory.py",
                &[],
                &[("make", &["loader"])],
            )],
            expected_problem: Some(
                "@factory 'make' in python/nodes/raw/x/factory.py makes only loaders; a factory \
                 that makes only loaders is declared in loaders.py",
            ),
        },
        NodeFilesTestCase {
            description: "a mixed factory outside factory.py",
            modules: vec![module(
                "python/nodes/raw/x/assets.py",
                &[],
                &[("make", &["asset", "check"])],
            )],
            expected_problem: Some(
                "@factory 'make' in python/nodes/raw/x/assets.py makes assets and checks; a \
                 factory that makes more than one kind is declared in factory.py",
            ),
        },
        NodeFilesTestCase {
            description: "two assets in asset.py",
            modules: vec![module(
                "python/nodes/s/o/asset.py",
                &[("a", "asset"), ("b", "asset")],
                &[],
            )],
            expected_problem: Some(
                "python/nodes/s/o/asset.py declares 2 nodes (a, b); asset.py declares exactly \
                 one, so give each its own unit folder or use a factory",
            ),
        },
        NodeFilesTestCase {
            description: "two declaration files in one folder",
            modules: vec![
                module("python/nodes/s/o/asset.py", &[("a", "asset")], &[]),
                module("python/nodes/s/o/loader.py", &[("b", "loader")], &[]),
            ],
            expected_problem: Some(
                "python/nodes/s/o/ holds asset.py and loader.py; a unit folder holds one node \
                 declaration file, plus checks.py",
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            node_file_problems(&test_case.modules).as_deref(),
            test_case.expected_problem,
            "{}",
            test_case.description
        );
    }
}
