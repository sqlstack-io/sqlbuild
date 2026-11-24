use std::path::PathBuf;

use crate::project_layout::_helpers::written::entries_holding;
use crate::project_layout::main::env_file_entries::env_file_entries;
use crate::project_layout::tests::test_types::{EnvFileEntriesTestCase, WrittenEntriesTestCase};

#[test]
fn given_paths_sqlbuild_writes_when_reading_entries_then_only_those_inside_the_project_count() {
    let test_cases = [
        WrittenEntriesTestCase {
            description: "a control store file and an output folder inside the project",
            written: &["custom-control.sqlite3", "output", "data/runs/output"],
            expected_entries: &["custom-control.sqlite3", "output", "data"],
        },
        WrittenEntriesTestCase {
            description: "the project itself and paths outside it",
            written: &[".", "..", "../elsewhere/output"],
            expected_entries: &[],
        },
    ];
    let root: PathBuf =
        std::env::temp_dir().join(format!("sqlbuild-written-entries-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&root);
    for test_case in test_cases {
        let written: Vec<PathBuf> = test_case.written.iter().map(PathBuf::from).collect();
        let entries: Vec<String> = entries_holding(&root, &written);
        assert_eq!(
            entries, test_case.expected_entries,
            "{}",
            test_case.description
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn given_env_file_settings_when_reading_entries_then_each_named_file_inside_the_project_counts() {
    let test_cases = [
        EnvFileEntriesTestCase {
            description: "the project's env file and a target's, one outside the project",
            config: "name = \"p\"\nadapter = \"duckdb\"\nenv_file = \"development.env\"\n\n\
                     [targets.prod]\nenv_file = \"secrets/prod.env\"\n\n\
                     [targets.ci]\nenv_file = \"../shared.env\"\n",
            expected_entries: &["development.env", "secrets"],
        },
        EnvFileEntriesTestCase {
            description: "loading turned off names nothing",
            config: "name = \"p\"\nadapter = \"duckdb\"\nenv_file = \"\"\n",
            expected_entries: &[],
        },
    ];
    let root: PathBuf =
        std::env::temp_dir().join(format!("sqlbuild-env-file-entries-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&root);
    for test_case in test_cases {
        std::fs::write(root.join("sqlbuild_project.toml"), test_case.config).expect("config");
        let entries: Vec<String> = env_file_entries(&root);
        assert_eq!(
            entries, test_case.expected_entries,
            "{}",
            test_case.description
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}
