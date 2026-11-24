use std::path::PathBuf;

use crate::project_layout::_helpers::written::entries_holding;
use crate::project_layout::tests::test_types::WrittenEntriesTestCase;

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
