use crate::planner_fixtures::_helpers::resolved_reads::resolved_reads;
use crate::planner_fixtures::_helpers::stub_markers::stub_markers;
use crate::planner_fixtures::main::prewarm_reads::prewarm_reads;
use crate::planner_fixtures::models::{FixtureReadCache, FixtureReadKey, FixtureReadReference};
use crate::planner_fixtures::tests::helpers::source_read_key;
use crate::planner_fixtures::tests::test_types::{
    FixtureMarkerTestCase, FixtureReadCacheTestCase, FixtureReadPrewarmTestCase,
};

#[test]
fn given_repeated_reads_when_resolving_then_full_reference_identity_controls_reuse() {
    let test_cases = [
        FixtureReadCacheTestCase {
            description: "identical request",
            sql: "SELECT order_id FROM __source(\"orders\")",
            package: None,
            argument_count: None,
            dialect: Some("duckdb"),
            expected_misses: 1,
        },
        FixtureReadCacheTestCase {
            description: "package changes",
            sql: "SELECT order_id FROM __source(\"orders\")",
            package: Some("shop"),
            argument_count: None,
            dialect: Some("duckdb"),
            expected_misses: 2,
        },
        FixtureReadCacheTestCase {
            description: "argument count changes",
            sql: "SELECT order_id FROM __source(\"orders\")",
            package: None,
            argument_count: Some(1),
            dialect: Some("duckdb"),
            expected_misses: 2,
        },
        FixtureReadCacheTestCase {
            description: "dialect changes",
            sql: "SELECT order_id FROM __source(\"orders\")",
            package: None,
            argument_count: None,
            dialect: None,
            expected_misses: 2,
        },
        FixtureReadCacheTestCase {
            description: "SQL changes",
            sql: "SELECT status FROM __source(\"orders\")",
            package: None,
            argument_count: None,
            dialect: Some("duckdb"),
            expected_misses: 2,
        },
    ];
    for test_case in test_cases {
        let key = FixtureReadKey {
            sql: "SELECT order_id FROM __source(\"orders\")".to_owned(),
            references: vec![FixtureReadReference {
                kind: "source".into(),
                name: "orders".into(),
                package: None,
                argument_count: None,
            }],
            dialect: Some("duckdb".into()),
        };
        let (cache, initial) = resolved_reads(FixtureReadCache::default(), &key);
        let (cache, repeated) = resolved_reads(cache, &key);
        assert_eq!(initial.ok(), repeated.ok(), "{}", test_case.description);
        assert_eq!(cache.misses, 1, "{}", test_case.description);
        let changed = FixtureReadKey {
            sql: test_case.sql.to_owned(),
            references: vec![FixtureReadReference {
                kind: "source".into(),
                name: "orders".into(),
                package: test_case.package.map(str::to_owned),
                argument_count: test_case.argument_count,
            }],
            dialect: test_case.dialect.map(str::to_owned),
        };
        let (cache, reads) = resolved_reads(cache, &changed);
        assert!(reads.is_ok(), "{}", test_case.description);
        assert_eq!(
            cache.misses, test_case.expected_misses,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_marker_text_when_stubbing_for_star_analysis_then_preserves_legacy_lexical_semantics() {
    let test_cases = [
        FixtureMarkerTestCase {
            description: "Unicode reference",
            sql: "SELECT * FROM __source(\"Straße\")",
            expected_sql: "SELECT * FROM Straße",
        },
        FixtureMarkerTestCase {
            description: "string and comment matches",
            sql: "SELECT '__ref(\"orders\")' -- __seed(\"customers\")",
            expected_sql: "SELECT 'orders' -- customers",
        },
        FixtureMarkerTestCase {
            description: "packaged reference and block comment",
            sql: "SELECT * FROM __dbt_ref(\"shop\", \"orders\") /* __source(\"customers\") */",
            expected_sql: "SELECT * FROM orders /* customers */",
        },
        FixtureMarkerTestCase {
            description: "unmatched marker spellings",
            sql: "SELECT * FROM __REF(\"orders\") JOIN __ref( 'customers' ) ON TRUE",
            expected_sql: "SELECT * FROM __REF(\"orders\") JOIN __ref( 'customers' ) ON TRUE",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            stub_markers(test_case.sql).ok().as_deref(),
            Some(test_case.expected_sql),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_prewarmed_reads_when_resolving_each_key_then_no_key_is_analysed_twice() {
    let test_cases = [
        FixtureReadPrewarmTestCase {
            description: "distinct keys",
            sqls: &[
                "SELECT order_id FROM __source(\"orders\")",
                "SELECT status FROM __source(\"orders\")",
            ],
            expected_misses: 2,
        },
        FixtureReadPrewarmTestCase {
            description: "repeated key",
            sqls: &[
                "SELECT order_id FROM __source(\"orders\")",
                "SELECT order_id FROM __source(\"orders\")",
            ],
            expected_misses: 1,
        },
    ];
    for test_case in test_cases {
        let keys: Vec<FixtureReadKey> = test_case
            .sqls
            .iter()
            .map(|sql| source_read_key(sql))
            .collect();
        let mut cache = prewarm_reads(FixtureReadCache::default(), &keys);
        assert_eq!(
            cache.misses, test_case.expected_misses,
            "{}",
            test_case.description
        );
        for key in &keys {
            let (updated, reads) = resolved_reads(cache, key);
            assert!(reads.is_ok(), "{}", test_case.description);
            cache = updated;
        }
        assert_eq!(
            cache.misses, test_case.expected_misses,
            "{}",
            test_case.description
        );
    }
}
