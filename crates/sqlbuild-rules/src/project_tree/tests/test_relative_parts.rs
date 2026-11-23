use std::path::{Path, PathBuf};

use crate::project_tree::errors::RelativePartsError;
use crate::project_tree::main::project_relative_parts::project_relative_parts;
use crate::project_tree::tests::test_types::RelativePartsTestCase;

#[test]
fn given_paths_when_made_project_relative_then_parts_follow_relative_to() {
    let test_cases = [
        RelativePartsTestCase {
            description: "a relative path keeps its parts",
            path: "models/../a.sql",
            resolved: "",
            under: None,
            expected_parts: Ok(vec!["models", "..", "a.sql"]),
        },
        RelativePartsTestCase {
            description: "an absolute path inside the project",
            path: "/project/link/a.sql",
            resolved: "/project/models/a.sql",
            under: Some("models"),
            expected_parts: Ok(vec!["a.sql"]),
        },
        RelativePartsTestCase {
            description: "an absolute path outside the project",
            path: "/elsewhere/a.sql",
            resolved: "/elsewhere/a.sql",
            under: None,
            expected_parts: Err(RelativePartsError::Escapes),
        },
        RelativePartsTestCase {
            description: "a path outside the requested folder",
            path: "models/a.sql",
            resolved: "",
            under: Some("rules"),
            expected_parts: Err(RelativePartsError::NotBeneath),
        },
        RelativePartsTestCase {
            description: "a relative path is beneath the current folder",
            path: "a.sql",
            resolved: "",
            under: Some("."),
            expected_parts: Ok(vec!["a.sql"]),
        },
    ];
    for test_case in test_cases {
        let parts = project_relative_parts(
            Path::new(test_case.path),
            Path::new("/project"),
            test_case.under.map(Path::new),
            |_| Ok::<PathBuf, ()>(PathBuf::from(test_case.resolved)),
        )
        .map(|parts| {
            parts
                .into_iter()
                .map(|part| part.to_string_lossy().into_owned())
                .collect::<Vec<String>>()
        });
        assert_eq!(
            parts
                .as_ref()
                .map(|parts| parts.iter().map(String::as_str).collect::<Vec<&str>>())
                .map_err(Clone::clone),
            test_case.expected_parts,
            "{}",
            test_case.description
        );
    }
}
