use crate::compiler::tests::helpers::{
    scanned_dialect_references, scanned_references, scanned_usages,
};
use crate::compiler::tests::test_types::DeclarationReferenceScanTestCase;
use crate::sql_scan::models::LexicalSyntax;

#[test]
fn given_sql_when_scanning_declaration_references_then_python_offsets_are_returned() {
    let test_cases = [
        DeclarationReferenceScanTestCase {
            description: "enum members and constants in order with code-point offsets",
            sql: "SELECT 'é', @enum(\"order_status\").PLACED, @const( 'sales_cap' ) FROM t",
            expected_references: Some("enum:order_status.PLACED@12..40, const:sales_cap@42..63"),
        },
        DeclarationReferenceScanTestCase {
            description: "references inside quotes, comments and dollar literals are text",
            sql: "SELECT '@const(\"a\")', `@const(\"b\")`, $tag$@const(\"c\")$tag$ -- @const(\"d\")\n/* @enum(\"e\").X */ @const(\"f\")",
            expected_references: Some("const:f@93..104"),
        },
        DeclarationReferenceScanTestCase {
            description: "a word after the keyword is not a reference and a $ inside a word is code",
            sql: "SELECT @constant, price$1, @const(\"cap\")",
            expected_references: Some("const:cap@27..40"),
        },
        DeclarationReferenceScanTestCase {
            description: "doubled single quotes stay inside the string",
            sql: "SELECT 'it''s @const(\"a\")', @const(\"b\")",
            expected_references: Some("const:b@28..39"),
        },
        DeclarationReferenceScanTestCase {
            description: "doubled backticks close and reopen quoted text, leaving it unclosed",
            sql: "SELECT 'it''s @const(\"a\")', `x``@const(\"b\")",
            expected_references: Some(" | stop:quote"),
        },
        DeclarationReferenceScanTestCase {
            description: "SQL without references is not scanned for unclosed quotes",
            sql: "SELECT 'unterminated",
            expected_references: Some(""),
        },
        DeclarationReferenceScanTestCase {
            description: "an unclosed quote before a reference stops the walk",
            sql: "SELECT @const(\"a\"), 'unterminated @const(\"b\")",
            expected_references: Some("const:a@7..18 | stop:quote"),
        },
        DeclarationReferenceScanTestCase {
            description: "a malformed enum reference stops the walk after earlier references",
            sql: "SELECT @const(\"a\"), @enum(\"order_status\"), @const(\"b\")",
            expected_references: Some("const:a@7..18 | stop:invalid enum"),
        },
        DeclarationReferenceScanTestCase {
            description: "a malformed constant reference at the end of the text stops the walk",
            sql: "SELECT @const",
            expected_references: Some(" | stop:invalid const"),
        },
        DeclarationReferenceScanTestCase {
            description: "Unicode whitespace inside and after the keyword matches Python's \\s",
            sql: "SELECT @enum\u{a0}(\u{2003}'order_status'\u{3000}) .\u{3000}PLACED",
            expected_references: Some("enum:order_status.PLACED@7..40"),
        },
        DeclarationReferenceScanTestCase {
            description: "a non-ASCII letter or number after the keyword continues the word",
            sql: "SELECT @consté, @enum\u{2167}, @const(\"a\")",
            expected_references: Some("const:a@24..35"),
        },
        DeclarationReferenceScanTestCase {
            description: "a non-ASCII symbol after the keyword ends the word, so the reference is malformed",
            sql: "SELECT @const\u{2192}",
            expected_references: Some(" | stop:invalid const"),
        },
        DeclarationReferenceScanTestCase {
            description: "an unclosed block comment before a reference stops the walk",
            sql: "SELECT /* open @const(\"a\")",
            expected_references: Some(" | stop:comment"),
        },
        DeclarationReferenceScanTestCase {
            description: "an unclosed dollar literal before a reference stops the walk",
            sql: "SELECT $tag$ @const(\"a\")",
            expected_references: Some(" | stop:quote"),
        },
    ];

    for test_case in test_cases {
        let scanned = scanned_references(test_case.sql);

        assert_eq!(
            scanned.as_deref(),
            test_case.expected_references,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sql_when_walking_declaration_usages_then_malformed_references_are_skipped() {
    let test_cases = [
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @enum('status').OPEN, @const('rate')",
            sql: "SELECT @enum('status').OPEN, @const('rate')",
            expected_references: Some("enum:status.OPEN@7..27, const:rate@29..43"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @enum('status') , @const('rate')",
            sql: "SELECT @enum('status') , @const('rate')",
            expected_references: Some("const:rate@25..39"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @enum x 'unclosed",
            sql: "SELECT @enum x 'unclosed",
            expected_references: Some(""),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @enum x, @const('rate') 'unclosed",
            sql: "SELECT @enum x, @const('rate') 'unclosed",
            expected_references: Some("const:rate@16..30"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @const(rate), @enum(\"size\").S",
            sql: "SELECT @const(rate), @enum(\"size\").S",
            expected_references: Some("enum:size.S@21..36"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT '@enum(''status'').A', @const('rate') /* open",
            sql: "SELECT '@enum(''status'').A', @const('rate') /* open",
            expected_references: Some("const:rate@30..44"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @enumx('status').A, @const_x('rate'), @consté, @const",
            sql: "SELECT @enumx('status').A, @const_x('rate'), @consté, @const('rate')",
            expected_references: Some("const:rate@54..68"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT $$ @enum('status').A $$, @enum('size').M -- @const('r",
            sql: "SELECT $$ @enum('status').A $$, @enum('size').M -- @const('rate')\n",
            expected_references: Some("enum:size.M@32..47"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT x$$ @const('rate') $$",
            sql: "SELECT x$$ @const('rate') $$",
            expected_references: Some("const:rate@11..25"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @enum ( 'status' ) . OPEN, @enum('status').OPEN, @con",
            sql: "SELECT @enum ( 'status' ) . OPEN, @enum('status').OPEN, @const('missing')",
            expected_references: Some(
                "enum:status.OPEN@7..32, enum:status.OPEN@34..54, const:missing@56..73",
            ),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT `@const('rate')`, \"@enum('size').A\"",
            sql: "SELECT `@const('rate')`, \"@enum('size').A\"",
            expected_references: Some(""),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @const('rate'), @const('rate'), 'é' @enum('size').XL",
            sql: "SELECT @const('rate'), @const('rate'), 'é' @enum('size').XL",
            expected_references: Some("const:rate@7..21, const:rate@23..37, enum:size.XL@43..59"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @@const, @enum, @const",
            sql: "SELECT @@const, @enum, @const",
            expected_references: Some(""),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT 1",
            sql: "SELECT 1",
            expected_references: Some(""),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @enum('status').OPEN /* @const('rate') */ 'it''s' @co",
            sql: "SELECT @enum('status').OPEN /* @const('rate') */ 'it''s' @const('rate')",
            expected_references: Some("enum:status.OPEN@7..27, const:rate@57..71"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @const('rate' ",
            sql: "SELECT @const('rate' ",
            expected_references: Some(""),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT /* x @const('rate')",
            sql: "SELECT /* x @const('rate')",
            expected_references: Some(" | stop:comment"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT 'a @enum('status').A",
            sql: "SELECT 'a @enum('status').A",
            expected_references: Some(" | stop:quote"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @enum x 'oops @const('rate')",
            sql: "SELECT @enum x 'oops @const('rate')",
            expected_references: Some(" | stop:quote"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @const('rate') $tag$ @enum('size').S",
            sql: "SELECT @const('rate') $tag$ @enum('size').S",
            expected_references: Some("const:rate@7..21 | stop:quote"),
        },
        DeclarationReferenceScanTestCase {
            description: "usage walk: SELECT @enum(\"status\").OPEN, @const('rate') /* tail @enum('s",
            sql: "SELECT @enum(\"status\").OPEN, @const('rate') /* tail @enum('size').S",
            expected_references: Some("enum:status.OPEN@7..27, const:rate@29..43 | stop:comment"),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            scanned_usages(test_case.sql).as_deref(),
            test_case.expected_references,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_backslash_escapes_when_scanning_declaration_references_then_dialect_quotes_are_text() {
    let syntax = LexicalSyntax {
        backslash_escape_quotes: vec!["'".to_owned()],
        line_comment_prefixes: vec!["--".to_owned(), "//".to_owned()],
        ..LexicalSyntax::default()
    };
    let test_cases = [
        DeclarationReferenceScanTestCase {
            description: "an escaped quote stays inside the string",
            sql: "SELECT 'O\\'Brien @const(\"a\")', @const(\"b\")",
            expected_references: Some("const:b@31..42"),
        },
        DeclarationReferenceScanTestCase {
            description: "a backslash at the end of a literal escapes itself",
            sql: "SELECT 'C:\\\\', @const(\"b\")",
            expected_references: Some("const:b@15..26"),
        },
        DeclarationReferenceScanTestCase {
            description: "a dialect line comment is text",
            sql: "SELECT 1 // it's @const(\"a\")\n, @const(\"b\")",
            expected_references: Some("const:b@31..42"),
        },
    ];

    for test_case in test_cases {
        let scanned = scanned_dialect_references(test_case.sql, &syntax);

        assert_eq!(
            scanned.as_deref(),
            test_case.expected_references,
            "{}",
            test_case.description
        );
    }
}
