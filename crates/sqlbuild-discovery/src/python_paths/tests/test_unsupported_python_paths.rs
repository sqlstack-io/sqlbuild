use std::fs;

use crate::python_paths::main::unsupported_python_paths::unsupported_python_paths;
use crate::python_paths::tests::test_types::PythonPathsTestCase;

#[test]
fn given_project_python_when_listing_unsupported_paths_then_only_undocumented_roots_remain() {
    let test_cases = [
        PythonPathsTestCase {
            description: "documented roots",
            files: &[
                "adapter.py",
                "python/orders.py",
                "orchestration/jobs.py",
                "orchestration/nested/calendar.py",
                "functions/python/f.py",
                "macros/m.py",
                "models/_macros/m.py",
                "models/a/_sqlbuild/hooks/python/h.py",
                "tests/unit/x.py",
                ".hidden/x.py",
                "target/x.py",
            ],
            expected_unsupported: &[],
        },
        PythonPathsTestCase {
            description: "undocumented roots sorted by parts",
            files: &[
                "a-b.py",
                "a/b.py",
                "models/x.py",
                "functions/sql/f.py",
                "helpers.py",
            ],
            expected_unsupported: &[
                "a/b.py",
                "a-b.py",
                "functions/sql/f.py",
                "helpers.py",
                "models/x.py",
            ],
        },
    ];
    for test_case in test_cases {
        let directory = std::env::temp_dir().join(format!(
            "sqlbuild-python-paths-{}-{}",
            std::process::id(),
            test_case.description.replace(' ', "-")
        ));
        let _ = fs::remove_dir_all(&directory);
        for file in test_case.files {
            let path = directory.join(file);
            let _ = fs::create_dir_all(path.parent().unwrap_or(&directory));
            let _ = fs::write(&path, "");
        }
        let unsupported: Vec<String> = unsupported_python_paths(&directory)
            .into_iter()
            .map(|parts| parts.join("/"))
            .collect();
        let _ = fs::remove_dir_all(&directory);
        assert_eq!(
            unsupported, test_case.expected_unsupported,
            "{}",
            test_case.description
        );
    }
}
