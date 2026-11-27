use crate::sql_scan::main::dialect_lexical_syntax::dialect_lexical_syntax;
use crate::sql_scan::main::dialect_non_code_ranges::dialect_non_code_ranges;
use crate::sql_scan::models::LexicalSyntax;
use crate::sql_scan::tests::helpers::{piece_texts, ranges_byte_by_byte};
use crate::sql_scan::tests::test_types::CandidateSkipTestCase;

#[test]
fn given_any_dialect_and_text_when_ranging_non_code_then_skipping_code_bytes_changes_nothing() {
    let test_cases = [
        CandidateSkipTestCase {
            description: "bigquery",
            syntax: dialect_lexical_syntax("bigquery"),
            expected_ranges: ranges_byte_by_byte,
        },
        CandidateSkipTestCase {
            description: "snowflake",
            syntax: dialect_lexical_syntax("snowflake"),
            expected_ranges: ranges_byte_by_byte,
        },
        CandidateSkipTestCase {
            description: "clickhouse",
            syntax: dialect_lexical_syntax("clickhouse"),
            expected_ranges: ranges_byte_by_byte,
        },
        CandidateSkipTestCase {
            description: "databricks",
            syntax: dialect_lexical_syntax("databricks"),
            expected_ranges: ranges_byte_by_byte,
        },
        CandidateSkipTestCase {
            description: "sqlserver",
            syntax: dialect_lexical_syntax("sqlserver"),
            expected_ranges: ranges_byte_by_byte,
        },
        CandidateSkipTestCase {
            description: "postgres",
            syntax: dialect_lexical_syntax("postgres"),
            expected_ranges: ranges_byte_by_byte,
        },
        CandidateSkipTestCase {
            description: "duckdb",
            syntax: dialect_lexical_syntax("duckdb"),
            expected_ranges: ranges_byte_by_byte,
        },
        CandidateSkipTestCase {
            description: "generic",
            syntax: dialect_lexical_syntax("generic"),
            expected_ranges: ranges_byte_by_byte,
        },
        CandidateSkipTestCase {
            description: "an empty line comment prefix makes every byte a candidate",
            syntax: LexicalSyntax {
                line_comment_prefixes: vec![String::new()],
                ..LexicalSyntax::default()
            },
            expected_ranges: ranges_byte_by_byte,
        },
    ];
    let pieces: [&str; 22] = [
        "a", " ", "\n", "'", "\"", "`", "/", "*", "/*", "*/", "--", "#", "# ", "#!", "//", "$",
        "$t$", "[", "]", "\\", "e'", "r'",
    ];
    let texts: Vec<String> = piece_texts(&pieces, 4);
    for test_case in test_cases {
        let mut ranged: usize = 0;
        for text in &texts {
            let ranges: Vec<(usize, usize)> = dialect_non_code_ranges(text, &test_case.syntax);
            assert_eq!(
                ranges,
                (test_case.expected_ranges)(text, &test_case.syntax),
                "{}: {text:?}",
                test_case.description
            );
            ranged += ranges.len();
        }
        assert!(
            ranged > 100_000,
            "{}: ranged {ranged} spans",
            test_case.description
        );
    }
}
