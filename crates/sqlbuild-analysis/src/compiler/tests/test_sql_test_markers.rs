use regex::Regex;

use crate::compiler::_helpers::sql_tests::markers::{
    in_protected_range, marker_names, replace_callable_markers, replace_named_markers,
};
use crate::compiler::tests::test_types::{
    CallableMarkerTestCase, MarkerNamesTestCase, ProtectedRangeTestCase,
};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

#[test]
fn given_markers_around_comments_and_strings_when_scanning_then_only_code_markers_are_used() {
    let test_cases = [
        MarkerNamesTestCase {
            description: "no markers leaves the SQL unchanged",
            sql: "SELECT 'a' AS label -- note\nFROM orders",
            expected_names: &[],
            expected_replaced_sql: "SELECT 'a' AS label -- note\nFROM orders",
        },
        MarkerNamesTestCase {
            description: "markers between many comments and strings",
            sql: "SELECT '__ref(\"quoted\")' AS a, /* __ref(\"hidden\") */ b\nFROM __ref(\"orders\") -- __ref(\"commented\")\nJOIN __ref(\"customers\") ON 'x' = 'y'\nWHERE c = '__ref(\"text\")' AND d IN (SELECT id FROM __ref(\"products\"))",
            expected_names: &["orders", "customers", "products"],
            expected_replaced_sql: "SELECT '__ref(\"quoted\")' AS a, /* __ref(\"hidden\") */ b\nFROM mock_orders -- __ref(\"commented\")\nJOIN mock_customers ON 'x' = 'y'\nWHERE c = '__ref(\"text\")' AND d IN (SELECT id FROM mock_products)",
        },
    ];
    let pattern = Regex::new(r#"(?i)__ref\("([^"]+)"\)"#).expect("valid marker pattern");
    let syntax = LexicalSyntax {
        line_comment_prefixes: vec!["--".to_string()],
        ..LexicalSyntax::default()
    };
    for test_case in test_cases {
        assert_eq!(
            marker_names(&pattern, &syntax, test_case.sql),
            test_case.expected_names,
            "{}",
            test_case.description
        );
        assert_eq!(
            replace_named_markers(test_case.sql, &pattern, &syntax, |name| Some(format!(
                "mock_{name}"
            ))),
            test_case.expected_replaced_sql,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_lateral_table_function_call_when_replacing_it_with_a_relation_then_lateral_goes_with_it()
{
    let test_cases = [
        CallableMarkerTestCase {
            description: "a mock after a comma LATERAL stands alone",
            sql: "SELECT l.a FROM b, LATERAL __table_fn(\"f\")(b.id) AS l",
            drop_lateral: true,
            expected_sql: "SELECT l.a FROM b, mock_f AS l",
        },
        CallableMarkerTestCase {
            description: "a mock after JOIN lateral, in any case, stands alone",
            sql: "SELECT l.a FROM b JOIN lateral __table_fn(\"f\")(b.id) AS l ON TRUE",
            drop_lateral: true,
            expected_sql: "SELECT l.a FROM b JOIN mock_f AS l ON TRUE",
        },
        CallableMarkerTestCase {
            description: "a rendered call keeps its LATERAL",
            sql: "SELECT l.a FROM b, LATERAL __table_fn(\"f\")(b.id) AS l",
            drop_lateral: false,
            expected_sql: "SELECT l.a FROM b, LATERAL mock_f AS l",
        },
        CallableMarkerTestCase {
            description: "lateral ending a comment before the call is kept",
            sql: "SELECT l.a FROM b, -- not lateral\n__table_fn(\"f\")(1) AS l",
            drop_lateral: true,
            expected_sql: "SELECT l.a FROM b, -- not lateral\nmock_f AS l",
        },
        CallableMarkerTestCase {
            description: "a word ending in lateral before the call is kept",
            sql: "SELECT l.a FROM collateral, __table_fn(\"f\")(1) AS l",
            drop_lateral: true,
            expected_sql: "SELECT l.a FROM collateral, mock_f AS l",
        },
    ];
    let pattern = Regex::new(r#"(?i)__table_fn\("([^"]+)"\)"#).expect("valid marker pattern");
    let syntax = LexicalSyntax {
        line_comment_prefixes: vec!["--".to_string()],
        ..LexicalSyntax::default()
    };
    for test_case in test_cases {
        let replaced = replace_callable_markers(
            test_case.sql,
            (&pattern, test_case.drop_lateral),
            &syntax,
            |name, _| Some((format!("mock_{name}"), true)),
        )
        .map(|(sql, _)| sql);
        assert_eq!(
            replaced,
            Ok(test_case.expected_sql.to_string()),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sorted_ranges_when_checking_offsets_then_only_offsets_inside_a_range_are_protected() {
    let test_cases = [
        ProtectedRangeTestCase {
            description: "no ranges protect nothing",
            ranges: &[],
            expected_protected: &[],
        },
        ProtectedRangeTestCase {
            description: "starts are inclusive and ends exclusive across several ranges",
            ranges: &[(2, 4), (6, 7), (10, 15)],
            expected_protected: &[2, 3, 6, 10, 11, 12, 13, 14],
        },
    ];
    for test_case in test_cases {
        let protected: Vec<usize> = (0..17)
            .filter(|index| in_protected_range(*index, test_case.ranges))
            .collect();
        assert_eq!(
            protected, test_case.expected_protected,
            "{}",
            test_case.description
        );
    }
}
