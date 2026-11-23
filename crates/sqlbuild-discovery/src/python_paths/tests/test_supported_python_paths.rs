use crate::python_paths::main::supported_python_paths::supported_python_paths;
use crate::python_paths::tests::test_types::SupportedPathsTestCase;

#[test]
fn given_project_python_when_listing_supported_paths_then_only_documented_roots_remain() {
    let test_cases = [
        SupportedPathsTestCase {
            description: "documented roots sorted by parts, skipping hidden, generated and undocumented files",
            files: &[
                "python/nodes/orders/main.py",
                "python/adapter.py",
                "orchestration/sensors/stock.py",
                "sql/functions/python/f.py",
                "sql/models/a/_sqlbuild/hooks/python/h.py",
                "helpers.py",
                "sql/models/x.py",
                ".hidden/x.py",
                "target/x.py",
                "python/nodes/__pycache__/main.py",
            ],
            expected_supported: &[
                "python/adapter.py",
                "sql/functions/python/f.py",
                "sql/models/a/_sqlbuild/hooks/python/h.py",
                "orchestration/sensors/stock.py",
                "python/nodes/orders/main.py",
            ],
        },
        SupportedPathsTestCase {
            description: "a project without Python",
            files: &[],
            expected_supported: &[],
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("temporary project");
        for file in test_case.files {
            let path = directory.path().join(file);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("folders");
            std::fs::write(&path, "").expect("file");
        }
        let supported: Vec<String> = supported_python_paths(directory.path())
            .into_iter()
            .map(|parts| parts.join("/"))
            .collect();
        assert_eq!(
            supported, test_case.expected_supported,
            "{}",
            test_case.description
        );
    }
}
