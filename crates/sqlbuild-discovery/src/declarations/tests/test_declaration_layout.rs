use crate::declarations::models::DeclarationKind;
use crate::declarations::tests::helpers::{layout_rows, owned_facts, owned_groups};
use crate::declarations::tests::test_types::LayoutTestCase;

#[test]
fn given_declaration_layouts_when_scanning_then_facts_and_failures_match_python() {
    let test_cases = [
        LayoutTestCase {
            kind: None,
            description: "global, inherited, grouped and local roots sort by relative path",
            files: &[
                "sql/macros/python/text.py",
                "sql/macros/python/__init__.py",
                "sql/macros/__init__.py",
                "sql/macros/sql/totals.sql",
                "sql/enums/status.sql",
                "sql/models/marts/_enums/local.sql",
                "sql/models/marts/_sqlbuild/constants/limits.sql",
                "sql/models/marts/_sqlbuild/audits/generic/positive.sql",
            ],
            directories: &[],
            expected_facts: Ok(&[
                (
                    "sql/enums/status.sql",
                    "enum",
                    "global",
                    "sql/enums",
                    None,
                    "sql/enums",
                ),
                (
                    "sql/macros/python/text.py",
                    "macro",
                    "global",
                    "sql/macros",
                    None,
                    "sql/macros",
                ),
                (
                    "sql/macros/sql/totals.sql",
                    "macro",
                    "global",
                    "sql/macros",
                    None,
                    "sql/macros",
                ),
                (
                    "sql/models/marts/_enums/local.sql",
                    "enum",
                    "local",
                    "sql/models",
                    Some("sql/models/marts"),
                    "sql/models/marts/_enums",
                ),
                (
                    "sql/models/marts/_sqlbuild/constants/limits.sql",
                    "constant",
                    "inherited",
                    "sql/models",
                    Some("sql/models/marts"),
                    "sql/models/marts/_sqlbuild/constants",
                ),
            ]),
            expected_groups: Ok(&[("sql/models", "sql/models/marts/_sqlbuild")]),
        },
        LayoutTestCase {
            kind: None,
            description: "a declaration group directly below a canonical root fails",
            files: &["sql/models/_sqlbuild/macros/text.py"],
            directories: &[],
            expected_facts: Err(
                "Grouped declaration root sql/models/_sqlbuild/ must be below a concrete owner \
                 directory; use the project-wide sql/macros/, sql/enums/, sql/constants/, sql/audits/, \
                 sql/schemas/, or sql/hooks/ root instead",
            ),
            expected_groups: Ok(&[]),
        },
        LayoutTestCase {
            kind: None,
            description: "a nested declaration root fails in both scans",
            files: &["sql/models/a/_enums/macros/x.py"],
            directories: &["sql/schemas/enums"],
            expected_facts: Err(
                "Declaration root sql/models/a/_enums/macros/ is nested inside another declaration \
                 tree",
            ),
            expected_groups: Err(
                "Declaration directory sql/schemas/enums/ is not allowed inside the project-wide \
                 sql/schemas/ role; scoped declarations belong under <folder>/_sqlbuild/ below a \
                 resource tree",
            ),
        },
        LayoutTestCase {
            kind: None,
            description: "unsupported group and role entries list every entry",
            files: &[
                "sql/models/a/_sqlbuild/notes.txt",
                "sql/audits/x.sql",
                "sql/audits/.keep",
            ],
            directories: &["sql/models/a/_sqlbuild/extra"],
            expected_facts: Err(
                "Declaration group sql/models/a/_sqlbuild/ contains unsupported entries: \
                 sql/models/a/_sqlbuild/extra, sql/models/a/_sqlbuild/notes.txt",
            ),
            expected_groups: Err(
                "Unsupported entries in sql/audits/: sql/audits/x.sql; audit files must live in \
                 sql/audits/generic/ or sql/audits/singular/",
            ),
        },
        LayoutTestCase {
            kind: None,
            description: "local singular audits and unknown hook roles fail the named layout",
            files: &[],
            directories: &[
                "sql/models/a/_sqlbuild/_audits/singular",
                "sql/models/b/_sqlbuild/hooks/x",
            ],
            expected_facts: Ok(&[]),
            expected_groups: Err(
                "sql/models/a/_sqlbuild/_audits/singular/ is invalid: singular audits are never used \
                 by name, so folder-only visibility has no meaning; use \
                 _sqlbuild/audits/singular/",
            ),
        },
        LayoutTestCase {
            kind: None,
            description: "a macro file directly below a macro root names its kind folder",
            files: &["sql/models/marts/_sqlbuild/_macros/text.py"],
            directories: &[],
            expected_facts: Err(
                "macro file sql/models/marts/_sqlbuild/_macros/text.py must live in \
                 sql/models/marts/_sqlbuild/_macros/python/ or sql/models/marts/_sqlbuild/_macros/sql/\n  \
                 --> sql/models/marts/_sqlbuild/_macros/text.py",
            ),
            expected_groups: Ok(&[("sql/models", "sql/models/marts/_sqlbuild")]),
        },
        LayoutTestCase {
            kind: None,
            description: "a macro file in the other kind folder fails",
            files: &["sql/macros/python/totals.sql"],
            directories: &[],
            expected_facts: Err(
                "macro file sql/macros/python/totals.sql must live in sql/macros/python/ or \
                 sql/macros/sql/\n  --> sql/macros/python/totals.sql",
            ),
            expected_groups: Ok(&[]),
        },
        LayoutTestCase {
            kind: None,
            description: "a scoped root at the project root fails",
            files: &["_macros/text.py"],
            directories: &[],
            expected_facts: Err(
                "Scoped declaration root _macros/ must be below a canonical authored root",
            ),
            expected_groups: Ok(&[]),
        },
        LayoutTestCase {
            kind: Some(DeclarationKind::Enum),
            description: "an isolated kind skips the misplaced roots of other kinds",
            files: &[
                "_macros/text.py",
                "sql/enums/status.sql",
                "sql/models/a/macros/x.py",
            ],
            directories: &[],
            expected_facts: Ok(&[(
                "sql/enums/status.sql",
                "enum",
                "global",
                "sql/enums",
                None,
                "sql/enums",
            )]),
            expected_groups: Ok(&[]),
        },
        LayoutTestCase {
            kind: Some(DeclarationKind::Constant),
            description: "an isolated kind still rejects a root nested in its own tree",
            files: &["sql/models/a/constants/_enums/x.sql"],
            directories: &[],
            expected_facts: Err(
                "Declaration root sql/models/a/constants/_enums/ is nested inside another \
                 declaration tree",
            ),
            expected_groups: Ok(&[]),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            layout_rows(&test_case),
            (
                owned_facts(test_case.expected_facts),
                owned_groups(test_case.expected_groups)
            ),
            "{}",
            test_case.description
        );
    }
}
