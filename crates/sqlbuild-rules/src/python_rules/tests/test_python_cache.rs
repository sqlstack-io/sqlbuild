use crate::tests::helpers::install_test_identity;
use serde_json::json;

use fensu_native::NativeProjectQuery;

use crate::python_rules::_helpers::engine::observations::relative_query;
use crate::python_rules::tests::helpers::{cached_runs, named_faults};
use crate::python_rules::tests::test_types::{
    CachedRuns, PythonCacheTestCase, RelativeQueryTestCase,
};

const UNANNOTATED: &str = "def send(count) -> None:\n    pass\n";
const ANNOTATED: &str = "def send(count: int) -> None:\n    pass\n";

#[test]
fn given_a_cached_python_run_when_running_again_then_only_changed_inputs_are_evaluated() {
    install_test_identity();
    let test_cases = [
        PythonCacheTestCase {
            description: "an unchanged project is answered from the cache",
            files: &[
                ("sql/hooks/python/notify.py", UNANNOTATED),
                ("python/providers/settings.py", ANNOTATED),
            ],
            config: json!({ "select": ["SQBRPYTHON101"] }),
            changes: &[],
            changed_config: json!({ "select": ["SQBRPYTHON101"] }),
            expected_runs: CachedRuns {
                first_faults: named_faults(&[("SQBRPYTHON101", "sql/hooks/python/notify.py")]),
                second_faults: named_faults(&[("SQBRPYTHON101", "sql/hooks/python/notify.py")]),
                second_hits: 2,
                second_misses: 0,
            },
        },
        PythonCacheTestCase {
            description: "an edited file is evaluated again and its old finding goes",
            files: &[
                ("sql/hooks/python/notify.py", UNANNOTATED),
                ("python/providers/settings.py", ANNOTATED),
            ],
            config: json!({ "select": ["SQBRPYTHON101"] }),
            changes: &[("sql/hooks/python/notify.py", ANNOTATED)],
            changed_config: json!({ "select": ["SQBRPYTHON101"] }),
            expected_runs: CachedRuns {
                first_faults: named_faults(&[("SQBRPYTHON101", "sql/hooks/python/notify.py")]),
                second_faults: named_faults(&[]),
                second_hits: 1,
                second_misses: 1,
            },
        },
        PythonCacheTestCase {
            description: "a changed option evaluates every file again",
            files: &[(
                "python/nodes/orders/main/build.py",
                "def build() -> None:\n    a: int = 1\n    b: int = a\n    print(b)\n",
            )],
            config: json!({ "select": ["SQBRPYTHON501"] }),
            changes: &[],
            changed_config: json!({
                "select": ["SQBRPYTHON501"],
                "rule_options": { "SQBRPYTHON501": { "max_statements": 2 } },
            }),
            expected_runs: CachedRuns {
                first_faults: named_faults(&[]),
                second_faults: named_faults(&[("SQBRPYTHON501", "python/nodes/orders/main/build.py")]),
                second_hits: 0,
                second_misses: 1,
            },
        },
        PythonCacheTestCase {
            description: "an unchanged file whose import target appears is evaluated again",
            files: &[(
                "python/nodes/orders/main/export.py",
                "from python.nodes.billing.main._totals import total\n",
            )],
            config: json!({ "select": ["SQBRPYTHON304"] }),
            changes: &[(
                "python/nodes/billing/main/_totals.py",
                "def total() -> int:\n    return 1\n",
            )],
            changed_config: json!({ "select": ["SQBRPYTHON304"] }),
            expected_runs: CachedRuns {
                first_faults: named_faults(&[]),
                second_faults: named_faults(&[("SQBRPYTHON304", "python/nodes/orders/main/export.py")]),
                second_hits: 0,
                second_misses: 2,
            },
        },
        PythonCacheTestCase {
            description: "an unchanged domain package whose folder gains a subdomain is evaluated again",
            files: &[
                ("python/nodes/inventory/__init__.py", ""),
                ("python/nodes/inventory/main/__init__.py", ""),
                (
                    "python/nodes/inventory/main/publish.py",
                    "def publish() -> None:\n    pass\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON613"] }),
            changes: &[
                ("python/nodes/inventory/restock/__init__.py", ""),
                ("python/nodes/inventory/restock/main/__init__.py", ""),
            ],
            changed_config: json!({ "select": ["SQBRPYTHON613"] }),
            expected_runs: CachedRuns {
                first_faults: named_faults(&[]),
                second_faults: named_faults(&[("SQBRPYTHON613", "python/nodes/inventory/__init__.py")]),
                second_hits: 0,
                second_misses: 1,
            },
        },
    ];
    for test_case in test_cases {
        let runs = cached_runs(
            test_case.files,
            test_case.config.clone(),
            test_case.changes,
            test_case.changed_config.clone(),
        );

        assert_eq!(
            runs,
            Ok(test_case.expected_runs),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_planned_project_query_when_caching_it_then_its_path_is_relative_to_the_project() {
    let test_cases = [
        RelativeQueryTestCase {
            description: "a POSIX path below the project",
            project_dir: "/work/proj",
            path: "/work/proj/python/inventory",
            expected_path: "python/inventory",
        },
        RelativeQueryTestCase {
            description: "a Windows project directory against Fensu's forward-slash path",
            project_dir: "C:\\work\\proj",
            path: "C:/work/proj/python/inventory",
            expected_path: "python/inventory",
        },
        RelativeQueryTestCase {
            description: "the project directory itself",
            project_dir: "/work/proj",
            path: "/work/proj",
            expected_path: ".",
        },
        RelativeQueryTestCase {
            description: "a sibling directory sharing the project's name prefix",
            project_dir: "/work/proj",
            path: "/work/proj2/python",
            expected_path: "/work/proj2/python",
        },
    ];
    for test_case in test_cases {
        let query = NativeProjectQuery {
            kind: "directory_entries".to_owned(),
            path: test_case.path.to_owned(),
            argument: String::new(),
        };

        let relative = relative_query(query, test_case.project_dir);

        assert_eq!(
            relative.path, test_case.expected_path,
            "{}",
            test_case.description
        );
    }
}
