use crate::tree::tests::helpers::{owned, wide_name_walk};
use crate::tree::tests::test_types::WideNameWalkTestCase;

#[test]
fn given_windows_names_with_lone_surrogates_when_walking_then_each_file_keeps_its_identity() {
    let test_cases = [WideNameWalkTestCase {
        description: "lone surrogates sharing a lossy name resolve to their own files",
        names: &[
            &[0x6F, 0x72, 0x64, 0x65, 0x72, 0x73, 0x2E, 0x73, 0x71, 0x6C],
            &[0x61, 0xDCE9, 0x2E, 0x73, 0x71, 0x6C],
            &[0x61, 0xD800, 0x2E, 0x73, 0x71, 0x6C],
            &[0x61, 0xFFFD, 0x2E, 0x73, 0x71, 0x6C],
        ],
        expected_paths: &[
            "models/a\\ud800.sql",
            "models/a\\udce9.sql",
            "models/a\u{FFFD}.sql",
            "models/orders.sql",
        ],
        expected_failures: &[true, true, false, false],
        expected_contents: &["file 2", "file 1", "file 3", "file 0"],
    }];
    for test_case in test_cases {
        assert_eq!(
            wide_name_walk(&test_case),
            (
                owned(test_case.expected_paths.iter().copied()),
                test_case.expected_failures.to_vec(),
                owned(test_case.expected_contents.iter().copied()),
            ),
            "{}",
            test_case.description
        );
    }
}
