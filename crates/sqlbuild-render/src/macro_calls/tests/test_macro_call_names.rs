use crate::macro_calls::errors::ScanError;
use crate::macro_calls::main::macro_call_names::macro_call_names;
use crate::macro_calls::tests::helpers::owned_names;
use crate::macro_calls::tests::test_types::MacroCallNamesTestCase;

#[test]
fn given_sql_when_listing_macro_call_names_then_python_scan_agrees() {
    let test_cases = [
        MacroCallNamesTestCase {
            description: "declaration calls are listed",
            sql: "@enum(x) @py.a(1)",
            expected_top_level: Ok(&["enum", "a"]),
            expected_nested: Ok(&["enum", "a"]),
        },
        MacroCallNamesTestCase {
            description: "a name without its parenthesis fails only the top-level scan",
            sql: "@py.ab c(1)",
            expected_top_level: Err(ScanError::MissingParenthesis),
            expected_nested: Ok(&["ab"]),
        },
        MacroCallNamesTestCase {
            description: "an at sign inside a word starts no call",
            sql: "x@\u{f6}re FROM o",
            expected_top_level: Ok(&[]),
            expected_nested: Ok(&[]),
        },
        MacroCallNamesTestCase {
            description: "an unclosed call fails only the top-level scan",
            sql: "@py.\u{e9}(1) @py.b(",
            expected_top_level: Err(ScanError::UnclosedParenthesis),
            expected_nested: Ok(&["\u{e9}", "b"]),
        },
        MacroCallNamesTestCase {
            description: "quoted, commented and doubled at signs start no call",
            sql: "'@py.q()' /* @py.c() */ @py.d ( 1 ) @@v",
            expected_top_level: Ok(&[]),
            expected_nested: Ok(&[]),
        },
        MacroCallNamesTestCase {
            description: "nested calls are listed only when nested",
            sql: "@py.a(@py.b(@py.c()))",
            expected_top_level: Ok(&["a"]),
            expected_nested: Ok(&["a", "b", "c"]),
        },
        MacroCallNamesTestCase {
            description: "SQL macro calls are listed with Python ones",
            sql: "@sql.a(1) @py.b(@sql.c())",
            expected_top_level: Ok(&["a", "b"]),
            expected_nested: Ok(&["a", "b", "c"]),
        },
        MacroCallNamesTestCase {
            description: "a call without a kind fails both scans",
            sql: "@py.a(1) @b(2)",
            expected_top_level: Err(ScanError::BareCall),
            expected_nested: Err(ScanError::BareCall),
        },
        MacroCallNamesTestCase {
            description: "an unclosed quote fails both scans",
            sql: "@py.a(\"x\") 'oops",
            expected_top_level: Err(ScanError::UnclosedQuote),
            expected_nested: Err(ScanError::UnclosedQuote),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            (
                macro_call_names(test_case.sql, false, None),
                macro_call_names(test_case.sql, true, None)
            ),
            (
                owned_names(test_case.expected_top_level),
                owned_names(test_case.expected_nested)
            ),
            "{}",
            test_case.description
        );
    }
}
