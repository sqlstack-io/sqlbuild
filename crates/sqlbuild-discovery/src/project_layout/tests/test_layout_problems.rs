use std::fs;

use crate::project_layout::main::layout_problems::layout_problems;
use crate::project_layout::tests::test_types::LayoutTestCase;

#[test]
fn given_project_entries_when_checking_layout_then_only_unexpected_entries_are_reported() {
    let test_cases = [
        LayoutTestCase {
            description: "expected layout",
            files: &[
                "sqlbuild_project.toml",
                "sqlbuild_local.toml",
                "README.md",
                "sql/models/a.sql",
                "sql/schemas/s.yml",
                "python/__init__.py",
                "python/adapter.py",
                "python/nodes/sales/orders/asset.py",
                "python/event_sinks/s.py",
                "rules/r.py",
                "tests/unit/t.sql",
                "warehouse.duckdb",
                "warehouse.duckdb.wal",
                "shop.sqlite-journal",
                "shop.db-wal",
                ".env",
            ],
            folders: &["orchestration", "target", "logs", ".agents", "__pycache__"],
            gitignore: "",
            extra_paths: &[],
            expected_paths: &[],
        },
        LayoutTestCase {
            description: "unknown entries at every checked level",
            files: &[
                "sqlbuild_project.toml",
                "notes.txt",
                "migrations/map.json",
                "models/a.sql",
                "sql/misc/x.sql",
                "sql/readme.md",
                "python/orders/asset.py",
                "python/helpers.py",
                "python/nodes/deep/unchecked/thing.json",
            ],
            folders: &[],
            gitignore: "",
            extra_paths: &[],
            expected_paths: &[
                "migrations/",
                "models/",
                "notes.txt",
                "python/helpers.py",
                "python/orders/",
                "sql/misc/",
                "sql/readme.md",
            ],
        },
        LayoutTestCase {
            description: "extra paths and gitignored entries are allowed",
            files: &[
                "sqlbuild_project.toml",
                "migrations/map.json",
                "sql/notes/n.md",
                "scratch/x.sql",
                "build.log",
            ],
            folders: &["data"],
            gitignore: "scratch/\n*.log\n",
            extra_paths: &["migrations/", "sql/notes", "data"],
            expected_paths: &[],
        },
    ];
    for test_case in test_cases {
        let directory = std::env::temp_dir().join(format!(
            "sqlbuild-project-layout-{}-{}",
            std::process::id(),
            test_case.description.replace(' ', "-")
        ));
        let _ = fs::remove_dir_all(&directory);
        let _ = fs::create_dir_all(&directory);
        let _ = fs::create_dir_all(directory.join(".git"));
        for file in test_case.files {
            let path = directory.join(file);
            let _ = fs::create_dir_all(path.parent().unwrap_or(&directory));
            let _ = fs::write(&path, "");
        }
        for folder in test_case.folders {
            let _ = fs::create_dir_all(directory.join(folder));
        }
        let _ = fs::write(directory.join(".gitignore"), test_case.gitignore);
        let extra: Vec<String> = test_case
            .extra_paths
            .iter()
            .map(|path| (*path).to_owned())
            .collect();
        let reported: Vec<String> = layout_problems(&directory, &extra)
            .into_iter()
            .map(|problem| problem.path)
            .collect();
        let _ = fs::remove_dir_all(&directory);
        assert_eq!(
            reported, test_case.expected_paths,
            "{}",
            test_case.description
        );
    }
}
