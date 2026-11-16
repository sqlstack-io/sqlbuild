use crate::semantic_validation::_helpers::normalization::normalize_native_sql_mapped;
use crate::semantic_validation::models::BindingPositions;
use crate::semantic_validation::tests::test_types::{
    MappedNormalizationTestCase, UniqueWordOffsetTestCase, UniqueWordScalingTestCase,
};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[test]
fn given_identifier_when_locating_unique_word_then_returns_its_char_offset() {
    let test_cases = [
        UniqueWordOffsetTestCase {
            description: "single whole-word occurrence",
            authored: "SELECT o.order_id, o.amount FROM orders AS o",
            identifier: "amount",
            expected_offset: Some(21),
        },
        UniqueWordOffsetTestCase {
            description: "occurrence inside a longer word is not a match",
            authored: "SELECT order_id_v2, order_id FROM orders",
            identifier: "order_id",
            expected_offset: Some(20),
        },
        UniqueWordOffsetTestCase {
            description: "repeated word is ambiguous",
            authored: "SELECT amount FROM orders WHERE amount > 0",
            identifier: "amount",
            expected_offset: None,
        },
        UniqueWordOffsetTestCase {
            description: "absent word",
            authored: "SELECT amount FROM orders",
            identifier: "quantity",
            expected_offset: None,
        },
        UniqueWordOffsetTestCase {
            description: "offsets count characters after non-ASCII text",
            authored: "SELECT 'café' AS label, total FROM orders",
            identifier: "total",
            expected_offset: Some(24),
        },
        UniqueWordOffsetTestCase {
            description: "identifier with non-word characters falls back to a substring scan",
            authored: "SELECT \"order total\" FROM orders",
            identifier: "order total",
            expected_offset: Some(8),
        },
    ];

    for test_case in test_cases {
        let located = BindingPositions {
            authored: test_case.authored.to_owned(),
            query_start: Some(0),
            lines: vec![0],
            cleaned_lines: vec![0],
            offsets: Vec::new(),
            passes: Vec::new(),
            words: OnceLock::new(),
        };
        for lookup in ["first", "cached"] {
            assert_eq!(
                located.unique_word_offset(test_case.identifier),
                test_case.expected_offset,
                "{} ({lookup} lookup)",
                test_case.description
            );
        }
    }
}

#[test]
fn given_many_unique_identifiers_when_locating_each_then_work_stays_linear() {
    let test_cases = [UniqueWordScalingTestCase {
        description: "one wide query with an unknown column per projection",
        identifiers: 20_000,
        expected_max_duration: Duration::from_secs(2),
    }];

    for test_case in test_cases {
        let located = BindingPositions {
            authored: (0..test_case.identifiers)
                .map(|index| format!("missing_{index}"))
                .collect::<Vec<_>>()
                .join(",\n"),
            query_start: Some(0),
            lines: vec![0],
            cleaned_lines: vec![0],
            offsets: Vec::new(),
            passes: Vec::new(),
            words: OnceLock::new(),
        };
        let started = Instant::now();
        let located_count = (0..test_case.identifiers)
            .filter(|index| {
                located
                    .unique_word_offset(&format!("missing_{index}"))
                    .is_some()
            })
            .count();
        assert_eq!(
            located_count, test_case.identifiers,
            "{}",
            test_case.description
        );
        assert!(
            started.elapsed() < test_case.expected_max_duration,
            "{}: took {:?}",
            test_case.description,
            started.elapsed()
        );
    }
}

#[test]
fn given_non_ascii_text_around_several_edits_when_normalizing_then_copied_characters_map_to_their_source()
-> Result<(), String> {
    let test_cases = [
        MappedNormalizationTestCase {
            description: "references between strings in other scripts",
            dialect: "duckdb",
            sql: "SELECT '\u{e9}' AS a, '\u{20ac}' AS b FROM __ref(\"orders\") AS o, \
                  __ref(\"customers\") AS c WHERE o.note = '\u{fc}' AND c.n = '\u{1f600}'",
            expected_fragment: "FROM orders AS o",
        },
        MappedNormalizationTestCase {
            description: "Snowflake variant paths with computed keys holding other scripts",
            dialect: "snowflake",
            sql: "SELECT '\u{e9}' AS a, v:a['x' || '\u{20ac}'] AS b, w:b['\u{df}' || 'k'] AS c \
                  FROM __ref(\"orders\") AS o WHERE o.n = '\u{fc}' || '\u{1f600}'",
            expected_fragment: "GET(",
        },
    ];
    for test_case in test_cases {
        let source: Vec<char> = test_case.sql.chars().collect();
        let (normalized, offsets) =
            normalize_native_sql_mapped(test_case.sql, test_case.dialect, "__stub_", &[])?;
        let output: Vec<char> = normalized.chars().collect();
        assert!(
            normalized.contains(test_case.expected_fragment),
            "{}: {normalized}",
            test_case.description
        );

        assert_eq!(offsets.len(), output.len() + 1, "{}", test_case.description);
        let copied: Vec<(char, Option<char>)> = output
            .iter()
            .zip(&offsets)
            .filter(|(character, _)| !character.is_ascii())
            .map(|(character, offset)| (*character, source.get(*offset).copied()))
            .collect();
        let authored: usize = source
            .iter()
            .filter(|character| !character.is_ascii())
            .count();
        assert_eq!(copied.len(), authored, "{}", test_case.description);
        for (character, mapped) in copied {
            assert_eq!(Some(character), mapped, "{}", test_case.description);
        }
    }
    Ok(())
}
