use crate::tests::helpers::install_test_identity;
use serde_json::json;

use crate::python_rules::tests::helpers::{owned_faults, python_rule_faults};
use crate::python_rules::tests::test_types::{PythonParseFailureTestCase, PythonRunTestCase};

#[test]
fn given_project_python_when_evaluating_python_rules_then_reports_fensu_findings_in_sqlbuild_terms()
{
    install_test_identity();
    let test_cases = [
        PythonRunTestCase {
            description: "a file outside python/nodes/ reports at a character column after non-ASCII text",
            files: &[(
                "sql/hooks/python/notify.py",
                "\"\"\"Notify.\"\"\"\n\n\ndef send(é: int, count) -> None:\n    pass\n",
            )],
            config: json!({ "select": ["SQBRPYTHON101"] }),
            expected_faults: vec![("SQBRPYTHON101", "sql/hooks/python/notify.py", 4, 18)],
        },
        PythonRunTestCase {
            description: "python/nodes/ holds only domains, utils/ and __init__.py; folders report on their anchor",
            files: &[
                ("python/__init__.py", ""),
                ("python/nodes/report.py", "\"\"\"Report.\"\"\"\n"),
                ("python/nodes/loaders/orders.py", "\"\"\"Orders.\"\"\"\n"),
                ("python/nodes/_helpers/Alpha.py", "\"\"\"Alpha.\"\"\"\n"),
                ("python/nodes/_helpers/__init__.py", ""),
                ("python/nodes/orders/daily/asset.py", "\"\"\"Daily.\"\"\"\n"),
                ("python/nodes/utils/currency/format.py", "\"\"\"Format.\"\"\"\n"),
            ],
            config: json!({ "select": ["SQBRPYTHON001"] }),
            expected_faults: vec![
                ("SQBRPYTHON001", "python/nodes/_helpers/__init__.py", 1, 1),
                ("SQBRPYTHON001", "python/nodes/loaders/orders.py", 1, 1),
                ("SQBRPYTHON001", "python/nodes/report.py", 1, 1),
            ],
        },
        PythonRunTestCase {
            description: "generic package names report once per package, except utils/ at the root",
            files: &[
                ("python/nodes/orders/common/b.py", "\"\"\"B.\"\"\"\n"),
                ("python/nodes/orders/common/a.py", "\"\"\"A.\"\"\"\n"),
                ("python/nodes/utils/currency/format.py", "\"\"\"Format.\"\"\"\n"),
                ("python/nodes/utils/shared/text.py", "\"\"\"Text.\"\"\"\n"),
                ("python/nodes/helpers/rows.py", "\"\"\"Rows.\"\"\"\n"),
            ],
            config: json!({ "select": ["SQBRPYTHON002"] }),
            expected_faults: vec![
                ("SQBRPYTHON002", "python/nodes/helpers/rows.py", 1, 1),
                ("SQBRPYTHON002", "python/nodes/orders/common/a.py", 1, 1),
                ("SQBRPYTHON002", "python/nodes/utils/shared/text.py", 1, 1),
            ],
        },
        PythonRunTestCase {
            description: "package-only rules skip files outside python/nodes/",
            files: &[
                ("python/nodes/report.py", "\"\"\"Report.\"\"\"\n"),
                ("python/providers/report.py", "\"\"\"Report.\"\"\"\n"),
            ],
            config: json!({ "select": ["SQBRPYTHON614"] }),
            expected_faults: vec![("SQBRPYTHON614", "python/nodes/report.py", 1, 1)],
        },
        PythonRunTestCase {
            description: "a unit main.py is an entry module holding one public function",
            files: &[
                (
                    "python/nodes/orders/daily/main.py",
                    "def first() -> None:\n    pass\n\n\ndef second() -> None:\n    pass\n",
                ),
                (
                    "python/nodes/orders/weekly/main.py",
                    "def only() -> None:\n    pass\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON617"] }),
            expected_faults: vec![("SQBRPYTHON617", "python/nodes/orders/daily/main.py", 1, 1)],
        },
        PythonRunTestCase {
            description: "a main-only threshold reads its option and skips helpers",
            files: &[
                (
                    "python/nodes/orders/main/build.py",
                    "def build() -> None:\n    a: int = 1\n    b: int = a\n    print(b)\n",
                ),
                (
                    "python/nodes/orders/_helpers/build_rows.py",
                    "def build_rows() -> None:\n    a: int = 1\n    b: int = a\n    print(b)\n",
                ),
            ],
            config: json!({
                "select": ["SQBRPYTHON501"],
                "rule_options": { "SQBRPYTHON501": { "max_statements": 2 } },
            }),
            expected_faults: vec![("SQBRPYTHON501", "python/nodes/orders/main/build.py", 1, 1)],
        },
        PythonRunTestCase {
            description: "a package-wide rule reports once per package",
            files: &[
                (
                    "python/nodes/orders/_helpers/first.py",
                    "def first() -> None:\n    pass\n",
                ),
                (
                    "python/nodes/orders/_helpers/second.py",
                    "def second() -> None:\n    pass\n",
                ),
            ],
            config: json!({
                "select": ["SQBRPYTHON610"],
                "rule_options": { "SQBRPYTHON610": { "max_helpers_container_modules": 1 } },
            }),
            expected_faults: vec![("SQBRPYTHON610", "python/nodes/orders/_helpers/first.py", 1, 1)],
        },
        PythonRunTestCase {
            description: "a public main entry nobody outside its domain imports",
            files: &[(
                "python/nodes/orders/main/export.py",
                "def export_orders() -> None:\n    pass\n",
            )],
            config: json!({ "select": ["SQBRPYTHON305"] }),
            expected_faults: vec![("SQBRPYTHON305", "python/nodes/orders/main/export.py", 1, 1)],
        },
        PythonRunTestCase {
            description: "a hook importing a public main entry counts as outside use",
            files: &[
                (
                    "python/nodes/orders/main/export.py",
                    "def export_orders() -> None:\n    pass\n",
                ),
                (
                    "sql/hooks/python/notify.py",
                    "from python.nodes.orders.main.export import export_orders\n\n\ndef notify() -> None:\n    export_orders()\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON305"] }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "naming contracts apply",
            files: &[(
                "python/providers/flags.py",
                "def is_ready() -> int:\n    return 1\n",
            )],
            config: json!({ "select": ["SQBRPYTHON402"] }),
            expected_faults: vec![("SQBRPYTHON402", "python/providers/flags.py", 1, 1)],
        },
        PythonRunTestCase {
            description: "imports resolve under the python package",
            files: &[
                (
                    "python/nodes/orders/main/export.py",
                    "from python.nodes.billing.main._totals import total\n",
                ),
                (
                    "python/nodes/billing/main/_totals.py",
                    "def total() -> int:\n    return 1\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON304"] }),
            expected_faults: vec![("SQBRPYTHON304", "python/nodes/orders/main/export.py", 1, 1)],
        },
        PythonRunTestCase {
            description: "a discarded result of a project function is found through the called module",
            files: &[
                (
                    "python/nodes/orders/main/export.py",
                    "from python.nodes.orders._helpers.totals import order_total\n\n\ndef export_orders() -> None:\n    order_total()\n",
                ),
                (
                    "python/nodes/orders/_helpers/totals.py",
                    "def order_total() -> int:\n    return 1\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON506"] }),
            expected_faults: vec![("SQBRPYTHON506", "python/nodes/orders/main/export.py", 5, 5)],
        },
        PythonRunTestCase {
            description: "node and hook functions are live, unreferenced helpers are dead, and hooks are never reported",
            files: &[
                (
                    "python/nodes/orders/daily/asset.py",
                    "@loader\ndef daily() -> None:\n    pass\n",
                ),
                (
                    "python/nodes/orders/_helpers/unused.py",
                    "def unused() -> None:\n    pass\n",
                ),
                (
                    "python/nodes/orders/_helpers/totals.py",
                    "def order_total() -> int:\n    return 1\n",
                ),
                (
                    "sql/hooks/python/notify.py",
                    "from python.nodes.orders._helpers.totals import order_total\n\n_UNUSED_LIMIT: int = 3\n\n\ndef notify() -> int:\n    return order_total()\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON306"] }),
            expected_faults: vec![("SQBRPYTHON306", "python/nodes/orders/_helpers/unused.py", 1, 1)],
        },
        PythonRunTestCase {
            description: "an audit factory under python/nodes/ is loaded",
            files: &[(
                "python/nodes/orders/quality/factories.py",
                "@audit_factory\ndef order_quality() -> list:\n    return []\n",
            )],
            config: json!({ "select": ["SQBRPYTHON306"] }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "a submodule imported from a folder without __init__.py is used",
            files: &[
                (
                    "python/nodes/orders/_tables.py",
                    "TABLES: int = 1\nUNUSED: int = 2\n",
                ),
                (
                    "python/nodes/orders/asset.py",
                    "from python.nodes.orders import _tables\n\n\n@asset\ndef count() -> int:\n    return _tables.TABLES\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON306"] }),
            expected_faults: vec![("SQBRPYTHON306", "python/nodes/orders/_tables.py", 2, 1)],
        },
    ];
    for test_case in test_cases {
        let faults = python_rule_faults(test_case.files, test_case.config.clone());

        assert_eq!(
            faults,
            Ok(owned_faults(&test_case.expected_faults)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_python_the_parser_rejects_when_evaluating_python_rules_then_the_run_fails_at_the_file() {
    install_test_identity();
    let test_cases = [PythonParseFailureTestCase {
        description: "a parameter list cut short",
        files: &[("python/nodes/orders/main/bad.py", "def broken(:\n")],
        expected_error: "python/nodes/orders/main/bad.py:1:12: cannot parse Python source",
    }];
    for test_case in test_cases {
        let faults = python_rule_faults(test_case.files, json!({ "select": ["SQBRPYTHON101"] }));

        assert!(
            faults
                .as_ref()
                .is_err_and(|error| error.contains(test_case.expected_error)),
            "{}: {faults:?}",
            test_case.description
        );
    }
}
