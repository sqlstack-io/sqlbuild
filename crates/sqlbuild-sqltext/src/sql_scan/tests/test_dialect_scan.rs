use crate::sql_scan::main::dialect_lexical_syntax::dialect_lexical_syntax;
use crate::sql_scan::main::dialect_non_code_end::dialect_non_code_end;
use crate::sql_scan::main::dialect_non_code_ranges::dialect_non_code_ranges;
use crate::sql_scan::models::{LexicalSyntax, Unclosed};
use crate::sql_scan::tests::helpers::{
    escape_strings, generic, raw_strings, single_quote_backslashes, triple_quotes,
};
use crate::sql_scan::tests::test_types::{
    DialectFirstNonCodeRangeTestCase, DialectNonCodeEndTestCase, DialectNonCodeRangesTestCase,
};

#[test]
fn given_an_offset_when_finding_dialect_non_code_end_then_only_opening_bytes_start_non_code() {
    let test_cases = [
        DialectNonCodeEndTestCase {
            description: "a code byte opens nothing",
            sql: "select 1",
            index: 0,
            syntax: generic(),
            expected_end: Ok(None),
        },
        DialectNonCodeEndTestCase {
            description: "a hash is code when it is not a line comment prefix",
            sql: "# note\nx",
            index: 0,
            syntax: generic(),
            expected_end: Ok(None),
        },
        DialectNonCodeEndTestCase {
            description: "a hash opens a line comment when the dialect says so",
            sql: "x # note\ny",
            index: 2,
            syntax: triple_quotes(),
            expected_end: Ok(Some(9)),
        },
        DialectNonCodeEndTestCase {
            description: "a lone slash is code",
            sql: "a / b",
            index: 2,
            syntax: generic(),
            expected_end: Ok(None),
        },
        DialectNonCodeEndTestCase {
            description: "a dollar tag opens a dollar-quoted string",
            sql: "$tag$ x $tag$ y",
            index: 0,
            syntax: generic(),
            expected_end: Ok(Some(13)),
        },
        DialectNonCodeEndTestCase {
            description: "a dollar parameter is code",
            sql: "$1 + 2",
            index: 0,
            syntax: generic(),
            expected_end: Ok(None),
        },
        DialectNonCodeEndTestCase {
            description: "an unclosed quote is reported",
            sql: "x 'open",
            index: 2,
            syntax: generic(),
            expected_end: Err(Unclosed::Quote),
        },
        DialectNonCodeEndTestCase {
            description: "the end of the text opens nothing",
            sql: "x",
            index: 1,
            syntax: generic(),
            expected_end: Ok(None),
        },
        DialectNonCodeEndTestCase {
            description: "an empty line comment prefix matches at every offset",
            sql: "ab\ncd",
            index: 1,
            syntax: LexicalSyntax {
                line_comment_prefixes: vec![String::new()],
                ..LexicalSyntax::default()
            },
            expected_end: Ok(Some(3)),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            dialect_non_code_end(test_case.sql.as_bytes(), test_case.index, &test_case.syntax),
            test_case.expected_end,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_dialect_lexical_syntax_when_scanning_non_code_then_dialect_decides_boundaries() {
    let test_cases = [
        DialectFirstNonCodeRangeTestCase {
            description: "generic sql ends a quote at a backslash-escaped quote",
            sql: r"'O\'Brien' x",
            syntax: generic(),
            expected_first_range: Some((0, 4)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "backslash escapes keep the quote open",
            sql: r"'O\'Brien' x",
            syntax: single_quote_backslashes(),
            expected_first_range: Some((0, 10)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "escape strings honour backslashes",
            sql: r"E'O\'Brien' x",
            syntax: escape_strings(),
            expected_first_range: Some((1, 11)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "plain strings keep backslashes literal under escape-string rules",
            sql: r"'C:\' x",
            syntax: escape_strings(),
            expected_first_range: Some((0, 5)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "raw strings disable backslash escapes",
            sql: r"r'C:\' x",
            syntax: raw_strings(),
            expected_first_range: Some((1, 6)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "triple quotes contain single quotes",
            sql: "'''it's''' x",
            syntax: triple_quotes(),
            expected_first_range: Some((0, 10)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "hash starts a line comment",
            sql: "# note\nx",
            syntax: triple_quotes(),
            expected_first_range: Some((0, 7)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "slash pair starts a line comment",
            sql: "// note\nx",
            syntax: single_quote_backslashes(),
            expected_first_range: Some((0, 8)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "nested block comments end at the outer close",
            sql: "/* a /* b */ c */ x",
            syntax: escape_strings(),
            expected_first_range: Some((0, 17)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "flat block comments end at the first close",
            sql: "/* a /* b */ c */ x",
            syntax: generic(),
            expected_first_range: Some((0, 12)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "a T-SQL bracket identifier hides the apostrophe inside it",
            sql: "[customer's name] x",
            syntax: dialect_lexical_syntax("tsql"),
            expected_first_range: Some((0, 17)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "a doubled closing bracket stays inside the identifier",
            sql: "[a]]'b] x",
            syntax: dialect_lexical_syntax("tsql"),
            expected_first_range: Some((0, 7)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "a comment prefix inside a bracket identifier opens no comment",
            sql: "[a--b] x",
            syntax: dialect_lexical_syntax("tsql"),
            expected_first_range: Some((0, 6)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "outside T-SQL a bracket is code, so the apostrophe opens a quote",
            sql: "[customer's name] x",
            syntax: generic(),
            expected_first_range: Some((9, 19)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "ClickHouse backslash escapes hold in double-quoted identifiers",
            sql: r#""a\"b" x"#,
            syntax: dialect_lexical_syntax("clickhouse"),
            expected_first_range: Some((0, 6)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "a ClickHouse hash before a word is code, not a comment",
            sql: "#x 'a'",
            syntax: dialect_lexical_syntax("clickhouse"),
            expected_first_range: Some((3, 6)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "a ClickHouse hash before a space opens a comment",
            sql: "# it's\nx",
            syntax: dialect_lexical_syntax("clickhouse"),
            expected_first_range: Some((0, 7)),
        },
        DialectFirstNonCodeRangeTestCase {
            description: "an unclosed backslash-escaped quote extends to the end",
            sql: r"'O\'",
            syntax: single_quote_backslashes(),
            expected_first_range: Some((0, 4)),
        },
    ];
    for test_case in test_cases {
        let ranges: Vec<(usize, usize)> = dialect_non_code_ranges(test_case.sql, &test_case.syntax);
        assert_eq!(
            ranges.first().copied(),
            test_case.expected_first_range,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_quoted_text_before_a_marker_when_collecting_non_code_ranges_then_dialect_decides_coverage()
{
    let test_cases = [
        DialectNonCodeRangesTestCase {
            description: "backslash escapes keep a later marker in code",
            sql: r#"SELECT 'O\'Brien' AS name FROM __ref("orders")"#,
            syntax: single_quote_backslashes(),
            expected_marker_protected: false,
        },
        DialectNonCodeRangesTestCase {
            description: "generic sql reads a later marker as quoted text",
            sql: r#"SELECT 'O\'Brien' AS name FROM __ref("orders")"#,
            syntax: generic(),
            expected_marker_protected: true,
        },
    ];
    for test_case in test_cases {
        let ranges: Vec<(usize, usize)> = dialect_non_code_ranges(test_case.sql, &test_case.syntax);
        let marker: usize = test_case.sql.find("__ref").unwrap_or_default();
        let protected: bool = ranges
            .iter()
            .any(|(start, end)| marker >= *start && marker < *end);
        assert_eq!(
            protected, test_case.expected_marker_protected,
            "{}",
            test_case.description
        );
    }
}
