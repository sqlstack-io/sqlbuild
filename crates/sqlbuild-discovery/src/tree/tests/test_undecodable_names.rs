use crate::tree::tests::helpers::{owned, undecodable_walk};
use crate::tree::tests::test_types::UndecodableWalkTestCase;

const RENAME: &str = " is not valid UTF-8; rename it so SQLBuild can read it";

#[test]
fn given_names_that_are_not_utf8_when_walking_then_each_file_keeps_its_own_identity() {
    let test_cases = [
        UndecodableWalkTestCase {
            description: "lossy names sort, resolve to real files and fail only when read",
            files: &[
                b"sql/models/orders.sql",
                b"sql/models/notes\xe9.txt",
                b"sql/models/old\xe9/inner.sql",
                b"sql/models/caf\xe9.sql",
            ],
            expected_paths: &[
                "sql/models/caf\\xe9.sql",
                "sql/models/old\\xe9/inner.sql",
                "sql/models/orders.sql",
            ],
            expected_failures: &[
                Some("Project path project/models/caf\\xe9.sql"),
                Some("Project path project/models/old\\xe9/inner.sql"),
                None,
            ],
            expected_contents: &["file 3", "file 2", "file 0"],
        },
        UndecodableWalkTestCase {
            description: "names sharing a lossy name stay distinct and a valid name is not blamed",
            files: &[
                b"sql/models/st\xe9.sql",
                b"sql/models/st\xe8.sql",
                b"sql/models/st\xef\xbf\xbd.sql",
                b"sql/models/a\xe9/x.sql",
                b"sql/models/a\xe8/y.sql",
            ],
            expected_paths: &[
                "sql/models/a\\xe8/y.sql",
                "sql/models/a\\xe9/x.sql",
                "sql/models/st\\xe8.sql",
                "sql/models/st\\xe9.sql",
                "sql/models/st\u{FFFD}.sql",
            ],
            expected_failures: &[
                Some("Project path project/models/a\\xe8/y.sql"),
                Some("Project path project/models/a\\xe9/x.sql"),
                Some("Project path project/models/st\\xe8.sql"),
                Some("Project path project/models/st\\xe9.sql"),
                None,
            ],
            expected_contents: &["file 4", "file 3", "file 1", "file 0", "file 2"],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            undecodable_walk(&test_case),
            (
                owned(test_case.expected_paths.iter().copied()),
                test_case
                    .expected_failures
                    .iter()
                    .map(|failure| failure.map(|prefix| format!("{prefix}{RENAME}")))
                    .collect::<Vec<Option<String>>>(),
                owned(test_case.expected_contents.iter().copied()),
            ),
            "{}",
            test_case.description
        );
    }
}
