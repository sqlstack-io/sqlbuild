use std::path::PathBuf;

use crate::digest::main::installed_code_facts::installed_code_facts;
use crate::digest::tests::helpers::write_file;
use crate::digest::tests::test_types::InstalledCodeFactsTestCase;

#[test]
fn given_search_paths_when_reading_installed_facts_then_importlib_metadata_is_reproduced() {
    let test_cases = [
        InstalledCodeFactsTestCase {
            description: "nothing installed is unknown and editable",
            files: &[(0, "other.txt", "")],
            expected_versions: ["unknown", "unknown"],
            expected_editable: true,
        },
        InstalledCodeFactsTestCase {
            description: "the first search path wins and names normalise",
            files: &[
                (
                    0,
                    "sqlbuild-1.2.0.dist-info/METADATA",
                    "Name: sqlbuild\nVersion: 1.2.0\n\nbody",
                ),
                (1, "sqlbuild-9.0.0.dist-info/METADATA", "Version: 9.0.0\n"),
                (1, "pyyaml-6.0.2.dist-info/METADATA", "version:  6.0.2 \n"),
            ],
            expected_versions: ["1.2.0", "6.0.2"],
            expected_editable: false,
        },
        InstalledCodeFactsTestCase {
            description: "an editable direct URL marks the install editable",
            files: &[
                (0, "sqlbuild-1.0.dist-info/METADATA", "Version: 1.0\n"),
                (
                    0,
                    "sqlbuild-1.0.dist-info/direct_url.json",
                    "{\"url\": \"file:///src\", \"dir_info\": {\"editable\": true}}",
                ),
            ],
            expected_versions: ["1.0", "unknown"],
            expected_editable: true,
        },
        InstalledCodeFactsTestCase {
            description: "an unreadable direct URL counts as editable",
            files: &[
                (0, "sqlbuild-1.0.dist-info/METADATA", "Version: 1.0\n"),
                (0, "sqlbuild-1.0.dist-info/direct_url.json", "not json"),
            ],
            expected_versions: ["1.0", "unknown"],
            expected_editable: true,
        },
    ];
    for test_case in test_cases {
        let directories = [
            tempfile::tempdir().expect("a temporary folder"),
            tempfile::tempdir().expect("a temporary folder"),
        ];
        for (index, relative_path, contents) in test_case.files {
            write_file(directories[*index].path(), relative_path, contents);
        }
        let search_paths: Vec<PathBuf> = directories
            .iter()
            .map(|directory| directory.path().to_path_buf())
            .collect();

        let (versions, editable) = installed_code_facts(&search_paths);

        assert_eq!(
            versions, test_case.expected_versions,
            "{}",
            test_case.description
        );
        assert_eq!(
            editable, test_case.expected_editable,
            "{}",
            test_case.description
        );
    }
}
