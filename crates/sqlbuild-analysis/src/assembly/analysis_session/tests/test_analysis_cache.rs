use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::models::NativeStore;

use crate::assembly::analysis_session::models::{ModelReference, SessionRequest};
use crate::assembly::analysis_session::tests::helpers::{
    amounts_family, cache_stats, cached_run, current_model_key, damaged, model_keys,
    model_requests, orders_request, pairs, second_model, session_lines, shapes, started,
    uncached_catalog,
};
use crate::assembly::analysis_session::tests::test_types::{
    CacheEditTestCase, CacheKeyTestCase, CacheReuseTestCase, CachedRun, DamagedCacheTestCase,
    ModelSpec, RelationDigestTestCase,
};

const STG_ORDERS: ModelSpec = (
    "stg_orders",
    "SELECT order_id, amount * 2 AS doubled FROM __source(\"raw_orders\")",
    &["raw_orders"],
    &[],
);
const ORDERS_MART: ModelSpec = (
    "orders_mart",
    "SELECT doubled, order_id + 1 AS next_id FROM __ref(\"stg_orders\")",
    &[],
    &["stg_orders"],
);
const ORDERS_STAR: ModelSpec = (
    "orders_star",
    "SELECT * FROM __ref(\"stg_orders\")",
    &[],
    &["stg_orders"],
);
const ORDERS_BAD: ModelSpec = (
    "orders_bad",
    "SELECT missing_column FROM __source(\"raw_orders\")",
    &["raw_orders"],
    &[],
);
const EVENTS: ModelSpec = (
    "events",
    "SELECT event_id FROM __source(\"raw_events\")",
    &["raw_events"],
    &[],
);
const EVENTS_MART: ModelSpec = (
    "events_mart",
    "SELECT event_id, 1 AS one FROM __ref(\"events\")",
    &[],
    &["events"],
);
const ORDERS: &[ModelSpec] = &[STG_ORDERS, ORDERS_MART, ORDERS_STAR, ORDERS_BAD];
/// Aggregates over a CTE of an open source, which the engine hands to the native legacy analysis.
const EVENTS_TOTAL: ModelSpec = (
    "events_total",
    "WITH e AS (SELECT event_id FROM __source(\"raw_events\")) \
     SELECT COUNT(event_id) AS events FROM e",
    &["raw_events"],
    &[],
);
/// Nests functions past the parser's depth guard, so its analysis fails.
const EVENTS_TOO_DEEP: ModelSpec = (
    "events_too_deep",
    concat!(
        "SELECT ",
        "ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(",
        "ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(",
        "ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(",
        "ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(ABS(event_id",
        "))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))",
        " AS deep FROM __source(\"raw_events\")",
    ),
    &["raw_events"],
    &[],
);

#[test]
fn given_a_filled_cache_when_rerunning_then_hits_match_the_uncached_session() {
    let test_cases = [
        CacheReuseTestCase {
            description: "typed chains, a star, a binding error and a native enrichment",
            models: ORDERS,
            expected_stats: [(0, 4, 4), (4, 0, 0)],
        },
        CacheReuseTestCase {
            description: "an open source's consumer takes the legacy analysis natively",
            models: &[EVENTS, EVENTS_MART],
            expected_stats: [(0, 2, 2), (2, 0, 0)],
        },
        CacheReuseTestCase {
            description: "non-ASCII models analysed natively are stored",
            models: &[
                (
                    "events",
                    "SELECT event_id AS \"gr\u{f6}\u{df}e\" FROM __source(\"raw_events\")",
                    &["raw_events"],
                    &[],
                ),
                (
                    "events_mart",
                    "SELECT \"gr\u{f6}\u{df}e\" FROM __ref(\"events\") \
                     WHERE \"gr\u{f6}\u{df}e\" IS NOT NULL",
                    &[],
                    &["events"],
                ),
            ],
            expected_stats: [(0, 2, 2), (2, 0, 0)],
        },
        CacheReuseTestCase {
            description: "a natively answered legacy fallback and a failed analysis are stored",
            models: &[STG_ORDERS, EVENTS_TOTAL, EVENTS_TOO_DEEP],
            expected_stats: [(0, 3, 3), (3, 0, 0)],
        },
    ];
    for test_case in test_cases {
        let uncached = session_lines(test_case.models);
        let cold: CachedRun = cached_run(
            orders_request(model_requests(test_case.models)),
            NativeStore::default(),
        );
        let cold_stats: (usize, usize, usize) = cache_stats(&cold);
        let cold_lines = cold.lines;
        let cold_catalog = cold.catalog;
        let warm: CachedRun =
            cached_run(orders_request(model_requests(test_case.models)), cold.store);

        assert_eq!(
            (&cold_lines, &warm.lines),
            (&uncached, &uncached),
            "{}",
            test_case.description
        );
        let uncached_changes = uncached_catalog(orders_request(model_requests(test_case.models)));
        assert_eq!(
            (&cold_catalog, &warm.catalog),
            (&uncached_changes, &uncached_changes),
            "{}",
            test_case.description
        );
        assert_eq!(
            [cold_stats, cache_stats(&warm)],
            test_case.expected_stats,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_an_edit_when_rerunning_then_only_changed_analyses_miss_and_match_uncached() {
    let test_cases = [
        CacheEditTestCase {
            description: "a comment changes one query but no shape",
            models: ORDERS,
            edited: &[
                (
                    "stg_orders",
                    "SELECT order_id, amount * 2 AS doubled FROM __source(\"raw_orders\") -- edit",
                    &["raw_orders"],
                    &[],
                ),
                ORDERS_MART,
                ORDERS_STAR,
                ORDERS_BAD,
            ],
            expected_hits: &[false, true, true, true],
        },
        CacheEditTestCase {
            description: "a new upstream column reaches every consumer's key",
            models: ORDERS,
            edited: &[
                (
                    "stg_orders",
                    "SELECT order_id, amount * 2 AS doubled, amount FROM __source(\"raw_orders\")",
                    &["raw_orders"],
                    &[],
                ),
                ORDERS_MART,
                ORDERS_STAR,
                ORDERS_BAD,
            ],
            expected_hits: &[false, false, false, true],
        },
        CacheEditTestCase {
            description: "an analysis hits when only a model it does not read changes",
            models: &[EVENTS, STG_ORDERS, EVENTS_MART],
            edited: &[
                EVENTS,
                (
                    "stg_orders",
                    "SELECT order_id, amount FROM __source(\"raw_orders\")",
                    &["raw_orders"],
                    &[],
                ),
                EVENTS_MART,
            ],
            expected_hits: &[true, false, true],
        },
        CacheEditTestCase {
            description: "an edit misses only the edited model beside legacy and failed analyses",
            models: &[STG_ORDERS, EVENTS_TOTAL, EVENTS_TOO_DEEP, ORDERS_MART],
            edited: &[
                (
                    "stg_orders",
                    "SELECT order_id, amount * 2 AS doubled FROM __source(\"raw_orders\") -- edit",
                    &["raw_orders"],
                    &[],
                ),
                EVENTS_TOTAL,
                EVENTS_TOO_DEEP,
                ORDERS_MART,
            ],
            expected_hits: &[false, true, true, true],
        },
    ];
    for test_case in test_cases {
        let cold: CachedRun = cached_run(
            orders_request(model_requests(test_case.models)),
            NativeStore::default(),
        );
        let edited: CachedRun =
            cached_run(orders_request(model_requests(test_case.edited)), cold.store);

        assert_eq!(
            edited.lines,
            session_lines(test_case.edited),
            "{}",
            test_case.description
        );
        assert_eq!(
            edited.catalog,
            uncached_catalog(orders_request(model_requests(test_case.edited))),
            "{}",
            test_case.description
        );
        assert_eq!(
            edited.hits, test_case.expected_hits,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_damaged_entries_when_rerunning_then_every_model_is_analysed_again() {
    let test_cases = [DamagedCacheTestCase {
        description: "every stored entry holds bytes no outcome encodes to",
        models: ORDERS,
        expected_stats: (0, 4, 4),
    }];
    for test_case in test_cases {
        let cold: CachedRun = cached_run(
            orders_request(model_requests(test_case.models)),
            NativeStore::default(),
        );
        let keys = cold.keys.clone();
        let warm: CachedRun = cached_run(
            orders_request(model_requests(test_case.models)),
            damaged(cold.store, &keys),
        );

        assert_eq!(
            warm.lines,
            session_lines(test_case.models),
            "{}",
            test_case.description
        );
        assert_eq!(
            cache_stats(&warm),
            test_case.expected_stats,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_any_analysis_input_changed_when_keying_then_the_key_changes() {
    let test_cases = [
        CacheKeyTestCase {
            description: "model name",
            change: |request| second_model(request).name = "orders_mart_v2".to_owned(),
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "query",
            change: |request| second_model(request).query_sql.push(' '),
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "placeholders",
            change: |request| second_model(request).placeholders = pairs(&[("region", "east")]),
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "reference analysis names",
            change: |request| {
                second_model(request).references.push(ModelReference {
                    analysis_name: "raw_orders".to_owned(),
                    model_ref: false,
                })
            },
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "reference kinds",
            change: |request| second_model(request).references[0].model_ref = false,
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "lineage references",
            change: |request| second_model(request).lineage_references[0].1 = "seed".to_owned(),
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "required relation names",
            change: |request| {
                second_model(request)
                    .required_names
                    .push("raw_orders".to_owned())
            },
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "CTE fact recovery",
            change: |request| second_model(request).recover_cte_facts = true,
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "set operation",
            change: |request| second_model(request).has_set_operation = true,
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "snapshot columns",
            change: |request| {
                second_model(request).snapshot_columns =
                    Some(("valid_from".to_owned(), "valid_to".to_owned()))
            },
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "pivot SQL",
            change: |request| second_model(request).pivot_sql.push(' '),
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "dynamic families",
            change: |request| {
                second_model(request)
                    .dynamic_families
                    .push(amounts_family())
            },
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "dialect",
            change: |request| request.dialect = "postgresql".to_owned(),
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "shape case sensitivity",
            change: |request| request.case_sensitive_shapes = true,
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "function return types",
            change: |request| request.function_return_types = pairs(&[("score", "DOUBLE")]),
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "nullability rules",
            change: |request| request.nullability_rules = None,
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "rich type inference",
            change: |request| request.rich_type_inference = false,
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "relation types",
            change: |request| {
                request.column_types = shapes(&[("raw_orders", &[("order_id", "BIGINT")])])
            },
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "relation nullability",
            change: |request| {
                request.column_nullability = shapes(&[(
                    "raw_orders",
                    &[("order_id", "unknown"), ("amount", "unknown")],
                )])
            },
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "complete binding schemas",
            change: |request| {
                request
                    .complete_schemas
                    .push(("raw_items".to_owned(), pairs(&[("id", "INTEGER")])))
            },
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "catalog schemas",
            change: |request| {
                request
                    .catalog_schemas
                    .push(("raw_items".to_owned(), pairs(&[("id", "INTEGER")])))
            },
            expected_key_changed: true,
        },
        CacheKeyTestCase {
            description: "dynamic families by table",
            change: |request| {
                request
                    .dynamic_families_by_table
                    .push(("raw_orders".to_owned(), vec![amounts_family()]))
            },
            expected_key_changed: true,
        },
    ];
    let base = model_keys(orders_request(model_requests(ORDERS)))[1];
    for test_case in test_cases {
        let mut request: SessionRequest = orders_request(model_requests(ORDERS));
        (test_case.change)(&mut request);

        assert_eq!(
            model_keys(request)[1] != base,
            test_case.expected_key_changed,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_any_relation_fact_changed_when_digesting_then_the_digest_changes() {
    let test_cases = [
        RelationDigestTestCase {
            description: "available types",
            change: |session| {
                session
                    .available_types
                    .set_default("raw_items", pairs(&[("id", "INTEGER")]))
            },
            subject: "raw_items",
            expected_changed: true,
        },
        RelationDigestTestCase {
            description: "available nullability",
            change: |session| {
                session
                    .available_nullability
                    .set_default("raw_items", pairs(&[("id", "non_null")]))
            },
            subject: "raw_items",
            expected_changed: true,
        },
        RelationDigestTestCase {
            description: "closed shape",
            change: |session| {
                session
                    .complete_shapes
                    .set_default("raw_items", pairs(&[("id", "INTEGER")]))
            },
            subject: "raw_items",
            expected_changed: true,
        },
        RelationDigestTestCase {
            description: "catalog schema",
            change: |session| {
                let _ = session
                    .catalog
                    .prepare(&[("", &shapes(&[("raw_items", &[("id", "INTEGER")])]))]);
            },
            subject: "raw_items",
            expected_changed: true,
        },
    ];
    for test_case in test_cases {
        let mut session = started(orders_request(model_requests(ORDERS)));
        let before = session.relation_digest(test_case.subject);
        (test_case.change)(&mut session);

        assert_eq!(
            session.relation_digest(test_case.subject) != before,
            test_case.expected_changed,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_read_relation_fact_changed_when_keying_then_the_reading_model_key_changes() {
    let test_cases = [RelationDigestTestCase {
        description: "upstream nullability with equal types",
        change: |session| {
            session
                .available_nullability
                .set_default("stg_orders", pairs(&[("order_id", "non_null")]))
        },
        subject: "orders_mart",
        expected_changed: true,
    }];
    for test_case in test_cases {
        let mut session = started(orders_request(model_requests(ORDERS)));
        let before: ContentDigest = current_model_key(&session, test_case.subject);
        (test_case.change)(&mut session);

        assert_eq!(
            current_model_key(&session, test_case.subject) != before,
            test_case.expected_changed,
            "{}",
            test_case.description
        );
    }
}
