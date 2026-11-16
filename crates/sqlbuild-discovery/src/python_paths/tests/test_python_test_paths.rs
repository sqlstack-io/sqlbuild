use crate::python_paths::main::python_test_paths::python_test_paths;
use crate::python_paths::main::supported_python_paths::supported_python_paths;
use crate::python_paths::tests::test_types::TestPathsTestCase;

#[test]
fn given_project_tests_when_listing_python_test_paths_then_only_mirrored_test_roots_remain() {
    let test_cases = [
        TestPathsTestCase {
            description: "tests/python, tests/providers and tests/orchestration, skipping SQL test folders, caches and hidden folders",
            files: &[
                "tests/orchestration/sensors/orders/test_restock.py",
                "tests/python/orders/import/test_main.py",
                "tests/providers/storefront/test_provider.py",
                "tests/python/orders/_test_types.py",
                "tests/rules/test_rule.py",
                "tests/unit/orders.py",
                "tests/python/__pycache__/test_main.py",
                "tests/python/.hidden/test_main.py",
                "python/orders/import/main.py",
            ],
            expected_tests: &[
                "tests/orchestration/sensors/orders/test_restock.py",
                "tests/providers/storefront/test_provider.py",
                "tests/python/orders/_test_types.py",
                "tests/python/orders/import/test_main.py",
            ],
        },
        TestPathsTestCase {
            description: "a project without Python tests",
            files: &["python/orders/import/main.py"],
            expected_tests: &[],
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("temporary project");
        for file in test_case.files {
            let path = directory.path().join(file);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("folders");
            std::fs::write(&path, "").expect("file");
        }
        let tests: Vec<String> = python_test_paths(directory.path())
            .into_iter()
            .map(|parts| parts.join("/"))
            .collect();
        let supported: Vec<Vec<String>> = supported_python_paths(directory.path());
        assert_eq!(tests, test_case.expected_tests, "{}", test_case.description);
        assert!(
            supported.iter().all(|parts| parts[0] != "tests"),
            "{}: discovery never reads tests/",
            test_case.description
        );
    }
}
