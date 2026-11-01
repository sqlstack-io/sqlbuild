use crate::semantic_validation::_helpers::diagnostics::map_findings;
use crate::semantic_validation::tests::test_types::MappedContextTestCase;
use polyglot_sql::{DialectType, ValidationError};

/// A diagnostic's line and column.
type Location = (Option<usize>, Option<usize>);

#[test]
fn given_an_unplaced_column_finding_when_mapping_then_its_place_and_clause_follow_the_sql() {
    let test_cases = [
        MappedContextTestCase {
            description: "a name once in WHERE on the third line",
            sql: "SELECT order_id\nFROM orders\nWHERE missing_total > 0",
            message: "Unknown column 'missing_total' in table 'orders'",
            expected_message: "Unknown column 'missing_total' in table 'orders' (context: WHERE)",
            expected_location: (Some(3), Some(7)),
        },
        MappedContextTestCase {
            description: "a name twice is not placed; the clause is its first one's",
            sql: "SELECT order_id FROM orders o JOIN customers c ON c.region = o.region WHERE region > 0",
            message: "Unknown column 'region' in table 'customers'",
            expected_message: "Unknown column 'region' in table 'customers' (context: JOIN ON)",
            expected_location: (None, None),
        },
        MappedContextTestCase {
            description: "ORDER BY inside OVER is the window's",
            sql: "SELECT ROW_NUMBER() OVER (ORDER BY shipped_rank) AS r FROM orders",
            message: "Unknown column 'shipped_rank' in table 'orders'",
            expected_message: "Unknown column 'shipped_rank' in table 'orders' (context: WINDOW ORDER BY)",
            expected_location: (Some(1), Some(36)),
        },
        MappedContextTestCase {
            description: "a name inside a longer word does not count",
            sql: "SELECT total_amount, amount FROM orders GROUP BY amount_band, amount",
            message: "Unknown column 'amount' in table 'orders'",
            expected_message: "Unknown column 'amount' in table 'orders' (context: SELECT)",
            expected_location: (None, None),
        },
    ];
    for test_case in test_cases {
        let mapped = map_findings(
            test_case.sql,
            DialectType::DuckDB,
            vec![(
                1,
                ValidationError::error(test_case.message.to_owned(), "E201"),
            )],
        )
        .unwrap_or_else(|error| panic!("{}: {error}", test_case.description));
        let found: Vec<(String, Location)> = mapped
            .into_iter()
            .map(|(_, error)| (error.message, (error.line, error.column)))
            .collect();
        assert_eq!(
            found,
            vec![(
                test_case.expected_message.to_owned(),
                test_case.expected_location
            )],
            "{}",
            test_case.description
        );
    }
}
