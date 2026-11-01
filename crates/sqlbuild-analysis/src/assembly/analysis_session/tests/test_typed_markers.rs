use sqlbuild_cache::store::models::NativeStore;

use crate::assembly::analysis_session::tests::helpers::{
    cached_run, completed, described, described_column_v, started, typed_marker_request,
};
use crate::assembly::analysis_session::tests::test_types::{
    CachedRun, TypedMarkerCacheTestCase, TypedMarkerTestCase,
};

#[test]
fn given_typed_markers_when_analysing_then_each_call_has_its_asserted_or_inferred_type() {
    let test_cases = [
        TypedMarkerTestCase {
            description: "an unknown body takes the asserted type",
            sql: "SELECT __polyglot_typed(bar(amount, 0, 10), 'VARCHAR') AS v FROM __ref(\"staged\")",
            expected_column: "v TEXT",
        },
        TypedMarkerTestCase {
            description: "a known body keeps its inferred type",
            sql: "SELECT __polyglot_typed(order_id, 'VARCHAR') AS v FROM __ref(\"staged\")",
            expected_column: "v INTEGER ",
        },
        TypedMarkerTestCase {
            description: "a known aliased column keeps its inferred type",
            sql: "SELECT __polyglot_typed(o.order_id, 'VARCHAR') AS v FROM __ref(\"staged\") AS o",
            expected_column: "v INTEGER ",
        },
        TypedMarkerTestCase {
            description: "a later set-operation branch types the union",
            sql: "SELECT CAST(NULL AS VARCHAR) AS v FROM __ref(\"staged\") UNION ALL \
                  SELECT __polyglot_typed(bar(amount, 0, 10), 'VARCHAR') AS v FROM __ref(\"staged\")",
            expected_column: "v TEXT",
        },
        TypedMarkerTestCase {
            description: "a NULL body takes the asserted type in a union",
            sql: "SELECT CAST(NULL AS BIGINT) AS v FROM __ref(\"staged\") AS o UNION ALL \
                  SELECT __polyglot_typed(NULL, 'BIGINT') AS v FROM __ref(\"staged\") AS o",
            expected_column: "v BIGINT",
        },
        TypedMarkerTestCase {
            description: "a CTE read by name",
            sql: "WITH c AS (SELECT __polyglot_typed(bar(amount, 0, 10), 'VARCHAR') AS v \
                  FROM __ref(\"staged\")) SELECT v FROM c",
            expected_column: "v TEXT",
        },
        TypedMarkerTestCase {
            description: "an operand of a larger expression",
            sql: "SELECT COALESCE(__polyglot_typed(bar(amount, 0, 10), 'VARCHAR'), 'x') AS v \
                  FROM __ref(\"staged\")",
            expected_column: "v TEXT",
        },
    ];

    for test_case in test_cases {
        let (_, outcomes, _) = completed(started(typed_marker_request(test_case.sql)));
        let lines: Vec<Vec<String>> = outcomes.iter().map(described).collect();
        let column: String = described_column_v(&lines);
        assert!(
            column.starts_with(test_case.expected_column),
            "{}: {column}",
            test_case.description,
        );
    }
}

#[test]
fn given_typed_markers_when_rerunning_from_the_cache_then_the_hit_matches_the_cold_run() {
    let test_cases = [
        TypedMarkerCacheTestCase {
            description: "an unknown body takes the asserted type",
            sql: "SELECT __polyglot_typed(bar(amount, 0, 10), 'VARCHAR') AS v FROM __ref(\"staged\")",
            expected_hits: &[true, true],
        },
        TypedMarkerCacheTestCase {
            description: "a known body keeps its inferred type",
            sql: "SELECT __polyglot_typed(order_id, 'VARCHAR') AS v FROM __ref(\"staged\")",
            expected_hits: &[true, true],
        },
        TypedMarkerCacheTestCase {
            description: "a later set-operation branch types the union",
            sql: "SELECT CAST(NULL AS VARCHAR) AS v FROM __ref(\"staged\") UNION ALL \
                  SELECT __polyglot_typed(bar(amount, 0, 10), 'VARCHAR') AS v FROM __ref(\"staged\")",
            expected_hits: &[true, true],
        },
        TypedMarkerCacheTestCase {
            description: "a CTE read by name",
            sql: "WITH c AS (SELECT __polyglot_typed(bar(amount, 0, 10), 'VARCHAR') AS v \
                  FROM __ref(\"staged\")) SELECT v FROM c",
            expected_hits: &[true, true],
        },
    ];

    for test_case in test_cases {
        let cold: CachedRun =
            cached_run(typed_marker_request(test_case.sql), NativeStore::default());
        let warm: CachedRun = cached_run(typed_marker_request(test_case.sql), cold.store);
        assert_eq!(warm.lines, cold.lines, "{}", test_case.description);
        assert_eq!(
            warm.hits, test_case.expected_hits,
            "{}",
            test_case.description
        );
    }
}
