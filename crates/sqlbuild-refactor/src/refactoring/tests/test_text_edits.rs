use crate::refactoring::_helpers::edits::text_edits::{
    identifier_sites, line_column, whole_word_offsets,
};
use crate::refactoring::_helpers::scanning::chars::chars;
use crate::refactoring::tests::helpers::{
    applied, grouped_changes, owned_changes, owned_sites, reference_edits,
};
use crate::refactoring::tests::test_types::{
    ApplyEditsTestCase, FileChangesTestCase, IdentifierSitesTestCase, LineColumnTestCase,
    WholeWordTestCase,
};

#[test]
fn given_edits_when_applying_then_python_order_and_overlap_rules_hold() {
    let test_cases = [
        ApplyEditsTestCase {
            description: "disjoint edits apply back to front",
            text: "SELECT a FROM t",
            edits: &[(7, 8, "b"), (14, 15, "u")],
            expected_text: Ok("SELECT b FROM u"),
        },
        ApplyEditsTestCase {
            description: "identical duplicates apply once",
            text: "SELECT a FROM t",
            edits: &[(7, 8, "b"), (7, 8, "b")],
            expected_text: Ok("SELECT b FROM t"),
        },
        ApplyEditsTestCase {
            description: "overlapping edits are refused at the earlier edit",
            text: "SELECT abc FROM t",
            edits: &[(7, 10, "x"), (9, 12, "y")],
            expected_text: Err("overlapping refactoring edits at offset 7"),
        },
        ApplyEditsTestCase {
            description: "two insertions at one offset keep Python's stable reverse order",
            text: "x",
            edits: &[(0, 0, "A"), (0, 0, "B")],
            expected_text: Ok("BAx"),
        },
        ApplyEditsTestCase {
            description: "offsets count code points",
            text: "é a",
            edits: &[(2, 3, "b")],
            expected_text: Ok("é b"),
        },
    ];
    for test_case in test_cases {
        let result: Result<String, String> = applied(
            test_case.text,
            &reference_edits(test_case.text, test_case.edits),
        )
        .map_err(|error| error.message);
        assert_eq!(
            result,
            test_case
                .expected_text
                .map(str::to_owned)
                .map_err(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_text_when_finding_identifier_sites_then_code_is_matched_ignoring_case() {
    let test_cases = [
        IdentifierSitesTestCase {
            description: "comments and strings are skipped, case is ignored",
            text: "WITH __ref__orders AS (SELECT 1) SELECT * FROM __REF__ORDERS -- __ref__orders\n'__ref__orders'",
            names: &["__ref__orders"],
            expected_sites: &[(5, 18, "__ref__orders"), (47, 60, "__ref__orders")],
        },
        IdentifierSitesTestCase {
            description: "the longest name wins",
            text: "x __ref__orders_v2 y",
            names: &["__ref__orders", "__ref__orders_v2"],
            expected_sites: &[(2, 18, "__ref__orders_v2")],
        },
        IdentifierSitesTestCase {
            description: "a name inside a longer identifier is not a site",
            text: "a__ref__orders $__ref__orders __ref__orders$",
            names: &["__ref__orders"],
            expected_sites: &[],
        },
    ];
    for test_case in test_cases {
        let names: Vec<String> = test_case
            .names
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        assert_eq!(
            identifier_sites(&chars(test_case.text), &names),
            owned_sites(test_case.expected_sites),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_text_when_finding_whole_words_then_identifier_neighbours_are_excluded() {
    let test_cases = [WholeWordTestCase {
        description: "longer identifiers and case variants are not the word",
        text: "amount, AMOUNT_total, x.amount",
        word: "amount",
        expected_offsets: &[0, 24],
    }];
    for test_case in test_cases {
        assert_eq!(
            whole_word_offsets(&chars(test_case.text), test_case.word),
            test_case.expected_offsets,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_offset_when_locating_then_line_and_column_are_one_based() {
    let test_cases = [
        LineColumnTestCase {
            description: "an offset after a line break",
            text: "ab\ncd",
            offset: 4,
            expected_position: (2, 2),
        },
        LineColumnTestCase {
            description: "the first offset",
            text: "ab\ncd",
            offset: 0,
            expected_position: (1, 1),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            line_column(&chars(test_case.text), test_case.offset),
            test_case.expected_position,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_edits_and_moves_when_grouping_then_changes_follow_final_paths() {
    let test_cases = [FileChangesTestCase {
        description: "a moved file's edits follow it and paths are sorted",
        edited_paths: &["sql/models/b.sql", "sql/models/a.sql"],
        moves: &[("sql/models/b.sql", "sql/models/z/b.sql")],
        expected_changes: &[
            ("sql/models/a.sql", "sql/models/a.sql", 1),
            ("sql/models/z/b.sql", "sql/models/b.sql", 1),
        ],
    }];
    for test_case in test_cases {
        assert_eq!(
            grouped_changes(test_case.edited_paths, test_case.moves),
            owned_changes(test_case.expected_changes),
            "{}",
            test_case.description
        );
    }
}
