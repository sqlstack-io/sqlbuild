use crate::model_validation::tests::helpers::{
    incremental, map, snapshot, strings, validation_outcome,
};
use crate::model_validation::tests::test_types::ValidateTestCase;
use crate::tests::test_types::Value;

#[test]
fn given_effective_configs_when_validating_natively_then_python_outcomes_result() {
    let test_cases = [
        ValidateTestCase {
            description: "a plain table with known references",
            config: vec![("materialized", Value::Str("table"))],
            references: &[
                "ref:orders",
                "seed:regions",
                "source:raw.orders",
                "udf:cents",
            ],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "an unknown model reference",
            config: vec![("materialized", Value::Str("table"))],
            references: &["ref:invoices"],
            query_sql: "select 1",
            expected_outcome: "Model file models/marts/orders_daily.sql references unknown model 'invoices'",
        },
        ValidateTestCase {
            description: "a table function called as a scalar function",
            config: vec![],
            references: &["udf:order_lines"],
            query_sql: "select 1",
            expected_outcome: "Model file models/marts/orders_daily.sql references table function 'order_lines' with __udf(); use __table_fn() in SQL contexts that support table-valued functions",
        },
        ValidateTestCase {
            description: "a model marked critical for review",
            config: vec![("critical", Value::Bool(true))],
            references: &[],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "a critical marker that is not a boolean",
            config: vec![("critical", Value::Str("yes"))],
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': critical must be a boolean",
        },
        ValidateTestCase {
            description: "a dbt reference its resolver accepts",
            config: vec![],
            references: &["dbt_ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "a rejected dbt reference stops at its index, after earlier references",
            config: vec![],
            references: &["ref:orders", "dbt_ref!:invoices", "ref:missing"],
            query_sql: "select 1",
            expected_outcome: "external 1",
        },
        ValidateTestCase {
            description: "an append incremental with ordered ISO bounds",
            config: incremental(vec![
                ("cursor_start", Value::Str("2024-01-01")),
                ("cursor_end", Value::Str("2024-02-01T00:00:00+00:00")),
                ("append_cursor_inclusive", Value::Bool(true)),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "cursor bounds out of order",
            config: incremental(vec![
                ("cursor_start", Value::Str("2024-02-01")),
                ("cursor_end", Value::Str("2024-01-01")),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start must be before exclusive cursor_end",
        },
        ValidateTestCase {
            description: "a start bound shifted before year 1 in UTC fails",
            config: incremental(vec![
                ("cursor_start", Value::Str("0001-01-01T00:00:00+01:00")),
                ("cursor_end", Value::Str("2024-01-01")),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start value '0001-01-01T00:00:00+01:00' falls outside years 1-9999 once converted to UTC",
        },
        ValidateTestCase {
            description: "an end bound shifted past year 9999 in UTC fails",
            config: incremental(vec![
                ("cursor_start", Value::Str("2024-01-01")),
                ("cursor_end", Value::Str("9999-12-31T23:59:00-01:00")),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_end value '9999-12-31T23:59:00-01:00' falls outside years 1-9999 once converted to UTC",
        },
        ValidateTestCase {
            description: "bounds at the edges of years 1 and 9999 in UTC",
            config: incremental(vec![
                ("cursor_start", Value::Str("0001-01-01T01:00:00+01:00")),
                ("cursor_end", Value::Str("9999-12-31T22:59:00-01:00")),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "an ISO week date, a basic date and a comma fraction are timestamps",
            config: incremental(vec![
                ("cursor_start", Value::Str("2024-W01-1")),
                ("cursor_end", Value::Str("20240102T000000,5")),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "a date object is a timestamp bound and orders by its text",
            config: incremental(vec![
                ("cursor_start", Value::Date("2024-03-01")),
                ("cursor_end", Value::Str("2024-02-01")),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start must be before exclusive cursor_end",
        },
        ValidateTestCase {
            description: "an unparsable timestamp names Python's repr",
            config: incremental(vec![("cursor_start", Value::Str("2024-01-01X1"))]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start value '2024-01-01X1' is not a valid ISO timestamp: Invalid isoformat string: '2024-01-01X1'",
        },
        ValidateTestCase {
            description: "a day past the month's end gives Python's range error",
            config: incremental(vec![("cursor_start", Value::Str("2023-02-29T10:00"))]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start value '2023-02-29T10:00' is not a valid ISO timestamp: day is out of range for month",
        },
        ValidateTestCase {
            description: "an offset of a day or more gives Python's timezone error",
            config: incremental(vec![("cursor_start", Value::Str("2024-01-01T00:00+24:00"))]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start value '2024-01-01T00:00+24:00' is not a valid ISO timestamp: offset must be a timedelta strictly between -timedelta(hours=24) and timedelta(hours=24), not datetime.timedelta(days=1).",
        },
        ValidateTestCase {
            description: "bounds under a second apart order by their float timestamps",
            config: incremental(vec![
                ("cursor_start", Value::Str("2024-01-01T00:00:00.000001")),
                ("cursor_end", Value::Str("2024-01-01T00:00:00.000002")),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "an integer cursor reads Decimal text with underscores and exponents",
            config: vec![
                ("materialized", Value::Str("incremental")),
                ("incremental_strategy", Value::Str("append")),
                ("cursor", Value::Str("order_id")),
                ("cursor_type", Value::Str("integer")),
                ("cursor_start", Value::Str(" 1_000 ")),
                ("cursor_end", Value::Str("1.5e3")),
            ],
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "integer bounds compare as decimals beyond 64 bits",
            config: vec![
                ("materialized", Value::Str("incremental")),
                ("incremental_strategy", Value::Str("append")),
                ("cursor", Value::Str("order_id")),
                ("cursor_type", Value::Str("integer")),
                ("cursor_start", Value::Str("100000000000000000000000")),
                ("cursor_end", Value::Str("99999999999999999999999")),
            ],
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start must be before exclusive cursor_end",
        },
        ValidateTestCase {
            description: "an integer cursor value of NaN is not a valid integer",
            config: vec![
                ("materialized", Value::Str("incremental")),
                ("incremental_strategy", Value::Str("append")),
                ("cursor", Value::Str("order_id")),
                ("cursor_type", Value::Str("integer")),
                ("cursor_start", Value::Str("NaN")),
            ],
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start value 'NaN' is not a valid integer",
        },
        ValidateTestCase {
            description: "an integer cursor value in non-ASCII digits is rejected",
            config: vec![
                ("materialized", Value::Str("incremental")),
                ("incremental_strategy", Value::Str("append")),
                ("cursor", Value::Str("order_id")),
                ("cursor_type", Value::Str("integer")),
                ("cursor_start", Value::Str("\u{663}")),
            ],
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start value '\u{663}' uses digits outside ASCII",
        },
        ValidateTestCase {
            description: "an integer cursor value beyond 64 bits is rejected",
            config: vec![
                ("materialized", Value::Str("incremental")),
                ("incremental_strategy", Value::Str("append")),
                ("cursor", Value::Str("order_id")),
                ("cursor_type", Value::Str("integer")),
                ("cursor_start", Value::BigInt("9223372036854775808")),
            ],
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start 9223372036854775808 is larger than a 64-bit integer",
        },
        ValidateTestCase {
            description: "a list cursor action is not an action",
            config: incremental(vec![("cursor_future_action", strings(&["cap"]))]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_future_action must be one of: cap, error",
        },
        ValidateTestCase {
            description: "a duration in non-ASCII digits is rejected",
            config: incremental(vec![("cursor_start_max_ahead", Value::Str("\u{663}d"))]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_start_max_ahead '\u{663}d' uses digits outside ASCII",
        },
        ValidateTestCase {
            description: "a duration with a trailing newline parses as Python's $ allows",
            config: incremental(vec![("cursor_start_max_ahead", Value::Str("2d\n"))]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "a placeholder name with Unicode word characters",
            config: vec![("materialized", Value::Str("table"))],
            references: &["ref:orders"],
            query_sql: "select @@@gr\u{f6}\u{df}e",
            expected_outcome: "model 'orders_daily': @@@placeholders are only allowed on custom materializations",
        },
        ValidateTestCase {
            description: "a timestamp cursor without a grain",
            config: vec![
                ("materialized", Value::Str("incremental")),
                ("incremental_strategy", Value::Str("append")),
                ("cursor", Value::Str("updated_at")),
                ("cursor_type", Value::Str("timestamp")),
            ],
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor_type=timestamp requires cursor_grain (valid values: day, hour, minute, month, second, year)",
        },
        ValidateTestCase {
            description: "a watermark microbatch with a satisfiable limit",
            config: incremental(vec![
                ("incremental_strategy", Value::Str("delete_insert")),
                ("incremental_mode", Value::Str("microbatch")),
                ("microbatch_strategy", Value::Str("watermark")),
                ("cursor_watermark_mode", Value::Str("all")),
                ("batch_size", Value::Str("1d")),
                ("lookback", Value::Str("3d")),
                (
                    "cursor_inputs",
                    map(vec![(
                        "orders",
                        map(vec![
                            ("column", Value::Str("updated_at")),
                            ("roles", strings(&["filter", "watermark"])),
                        ]),
                    )]),
                ),
                (
                    "microbatch_limit",
                    map(vec![
                        ("max_batches", Value::Int(5)),
                        ("action", Value::Str("cap_from_start")),
                    ]),
                ),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "a watermark limit below the lookback requirement",
            config: incremental(vec![
                ("incremental_strategy", Value::Str("delete_insert")),
                ("incremental_mode", Value::Str("microbatch")),
                ("microbatch_strategy", Value::Str("watermark")),
                ("cursor_watermark_mode", Value::Str("all")),
                ("batch_size", Value::Str("1d")),
                ("lookback", Value::Str("3d")),
                (
                    "cursor_inputs",
                    map(vec![(
                        "orders",
                        map(vec![
                            ("column", Value::Str("updated_at")),
                            ("roles", strings(&["watermark"])),
                        ]),
                    )]),
                ),
                ("max_microbatches", Value::Int(3)),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': max_microbatches 3 is below the ordinary lookback requirement of 4 batches",
        },
        ValidateTestCase {
            description: "merge exclusions overlapping the unique key",
            config: incremental(vec![
                ("incremental_strategy", Value::Str("merge")),
                ("unique_key", Value::Str("Order_Id")),
                ("merge_exclude_columns", strings(&["order_id"])),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': merge_exclude_columns cannot include unique_key column(s): order_id",
        },
        ValidateTestCase {
            description: "delete_insert replacing groups of rows by replace_key",
            config: vec![
                ("materialized", Value::Str("incremental")),
                ("incremental_strategy", Value::Str("delete_insert")),
                ("replace_key", strings(&["forecast_version"])),
            ],
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "delete_insert without a cursor or a key",
            config: vec![
                ("materialized", Value::Str("incremental")),
                ("incremental_strategy", Value::Str("delete_insert")),
            ],
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': delete_insert without cursor requires unique_key or replace_key",
        },
        ValidateTestCase {
            description: "replace_key beside unique_key",
            config: incremental(vec![
                ("incremental_strategy", Value::Str("delete_insert")),
                ("unique_key", Value::Str("order_id")),
                ("replace_key", Value::Str("forecast_version")),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': set either unique_key (one row per key) or replace_key (groups of rows), not both",
        },
        ValidateTestCase {
            description: "replace_key on a merge",
            config: incremental(vec![
                ("incremental_strategy", Value::Str("merge")),
                ("unique_key", Value::Str("order_id")),
                ("replace_key", Value::Str("forecast_version")),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': replace_key requires incremental_strategy=delete_insert",
        },
        ValidateTestCase {
            description: "replace_key on a microbatch",
            config: incremental(vec![
                ("incremental_strategy", Value::Str("delete_insert")),
                ("incremental_mode", Value::Str("microbatch")),
                ("replace_key", Value::Str("forecast_version")),
                ("microbatch_strategy", Value::Str("watermark")),
                ("cursor_watermark_mode", Value::Str("all")),
                ("batch_size", Value::Str("1d")),
                ("lookback", Value::Str("3d")),
                (
                    "cursor_inputs",
                    map(vec![(
                        "orders",
                        map(vec![
                            ("column", Value::Str("updated_at")),
                            ("roles", strings(&["filter", "watermark"])),
                        ]),
                    )]),
                ),
                (
                    "microbatch_limit",
                    map(vec![
                        ("max_batches", Value::Int(5)),
                        ("action", Value::Str("cap_from_start")),
                    ]),
                ),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': replace_key is not supported with incremental_mode=microbatch; each batch only replaces its own window",
        },
        ValidateTestCase {
            description: "replace_key on a table",
            config: vec![
                ("materialized", Value::Str("table")),
                ("replace_key", Value::Str("forecast_version")),
            ],
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': replace_key is only valid for incremental models",
        },
        ValidateTestCase {
            description: "an empty replace_key",
            config: incremental(vec![
                ("incremental_strategy", Value::Str("delete_insert")),
                ("replace_key", strings(&[])),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': replace_key must be a column name or a list of column names",
        },
        ValidateTestCase {
            description: "a bounded replay policy",
            config: incremental(vec![("replay_on_change", Value::Str("bounded- 14d"))]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "an incremental-only key on a table",
            config: vec![
                ("materialized", Value::Str("table")),
                ("full_refresh", Value::Bool(true)),
            ],
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': full_refresh is only valid for incremental models",
        },
        ValidateTestCase {
            description: "a timestamp snapshot with observed history",
            config: snapshot(vec![
                ("observed_at", Value::Str("loaded_at")),
                ("historical_input", Value::Str("snapshot")),
                ("invalidate_hard_deletes", Value::Bool(true)),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "a check snapshot mixing the wildcard",
            config: snapshot(vec![
                ("snapshot_strategy", Value::Str("check")),
                ("check_columns", strings(&["*", "status"])),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': check_columns [*] cannot be combined with explicit columns",
        },
        ValidateTestCase {
            description: "an enforced contract naming an undeclared unique key",
            config: snapshot(vec![
                ("contract", Value::Str("enforced")),
                ("columns", map(vec![("updated_at", Value::Null)])),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': unique_key references column 'order_id' not declared in enforced contract",
        },
        ValidateTestCase {
            description: "a custom materialization with matching placeholders",
            config: vec![
                ("materialized", Value::Str("ledger")),
                ("placeholders", map(vec![("region", Value::Str("emea"))])),
            ],
            references: &[],
            query_sql: "select @@@region, '@@@' as marker",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "placeholders on a built-in materialization",
            config: vec![("materialized", Value::Str("view"))],
            references: &[],
            query_sql: "select @@@region",
            expected_outcome: "model 'orders_daily': @@@placeholders are only allowed on custom materializations",
        },
        ValidateTestCase {
            description: "an unknown custom materialization",
            config: vec![("materialized", Value::Str("archive"))],
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': unknown materialization 'archive'; not a built-in type and no custom materialization with that name was discovered",
        },
        ValidateTestCase {
            description: "a table migrating from an old name with an old-name view",
            config: vec![
                ("materialized", Value::Str("table")),
                ("migrate_from", Value::Str(" legacy_orders ")),
                ("old_name_view", Value::Str("7d")),
            ],
            references: &[],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "a view forcing a migration",
            config: vec![
                ("materialized", Value::Str("view")),
                ("migrate_from", Value::Str("legacy_orders")),
                ("migrate_force", Value::Bool(true)),
            ],
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': migrate_force is only valid for incremental and snapshot models; nothing is replaced when a 'view' model migrates, because tables and views are rebuilt under their new name",
        },
        ValidateTestCase {
            description: "an old-name view without a duration",
            config: vec![("old_name_view", Value::Null)],
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': old_name_view must be a positive duration such as 7d, or false; got None",
        },
        ValidateTestCase {
            description: "concurrent batches without the project capability",
            config: incremental(vec![("batch_concurrency", Value::Int(2))]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': batch_concurrency > 1 requires incremental_mode=microbatch",
        },
    ];

    for test_case in test_cases {
        let outcome =
            validation_outcome(test_case.config, test_case.references, test_case.query_sql);

        assert_eq!(
            outcome, test_case.expected_outcome,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_maintained_model_headers_when_validating_then_refresh_fields_are_checked() {
    let maintained = |extra: Vec<(&'static str, Value)>| {
        let mut config = vec![("materialized", Value::Str("maintained"))];
        config.extend(extra);
        config
    };
    let test_cases = [
        ValidateTestCase {
            description: "a maintained model with a lag, a mode and a warehouse",
            config: maintained(vec![
                ("refresh", map(vec![("lag", Value::Str("1h"))])),
                ("refresh_mode", Value::Str("adaptive")),
                ("refresh_warehouse", Value::Str("transform_wh")),
                ("refresh_on_build", Value::Bool(true)),
            ]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "a maintained model refreshed by its readers",
            config: maintained(vec![]),
            references: &["ref:orders"],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "refresh on a table",
            config: vec![
                ("materialized", Value::Str("table")),
                ("refresh", map(vec![("lag", Value::Str("1h"))])),
            ],
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh is only valid for maintained models",
        },
        ValidateTestCase {
            description: "refresh_on_build on a model with no materialization",
            config: vec![("refresh_on_build", Value::Bool(true))],
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh_on_build is only valid for maintained models",
        },
        ValidateTestCase {
            description: "a refresh block with a form no adapter has",
            config: maintained(vec![("refresh", map(vec![("every", Value::Str("1h"))]))]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh takes exactly one form, such as refresh (lag 1h); got: every",
        },
        ValidateTestCase {
            description: "a refresh block with two forms",
            config: maintained(vec![(
                "refresh",
                map(vec![
                    ("lag", Value::Str("1h")),
                    ("lag_cap", Value::Str("2h")),
                ]),
            )]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh takes exactly one form, such as refresh (lag 1h); got: lag, lag_cap",
        },
        ValidateTestCase {
            description: "refresh written as a plain value",
            config: maintained(vec![("refresh", Value::Str("1h"))]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh must be a block such as refresh (lag 1h)",
        },
        ValidateTestCase {
            description: "a lag in months",
            config: maintained(vec![("refresh", map(vec![("lag", Value::Str("1mo"))]))]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh lag must be a fixed duration such as 5m, 1h or 1d; got '1mo'",
        },
        ValidateTestCase {
            description: "a lag that is not a duration",
            config: maintained(vec![("refresh", map(vec![("lag", Value::Str("hourly"))]))]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh lag must be a fixed duration such as 5m, 1h or 1d; got 'hourly'",
        },
        ValidateTestCase {
            description: "an unknown refresh mode",
            config: maintained(vec![("refresh_mode", Value::Str("fast"))]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh_mode must be one of: adaptive, auto, full, incremental; got 'fast'",
        },
        ValidateTestCase {
            description: "refresh_on_build that is not a boolean",
            config: maintained(vec![("refresh_on_build", Value::Str("yes"))]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh_on_build must be a boolean",
        },
        ValidateTestCase {
            description: "a blank refresh warehouse",
            config: maintained(vec![("refresh_warehouse", Value::Str("  "))]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh_warehouse must be a non-empty warehouse name",
        },
        ValidateTestCase {
            description: "an incremental-only key on a maintained model",
            config: maintained(vec![("cursor", Value::Str("updated_at"))]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': cursor is not valid for maintained models: the warehouse refreshes them from the whole query",
        },
        ValidateTestCase {
            description: "the smallest lag Snowflake accepts",
            config: maintained(vec![("refresh", map(vec![("lag", Value::Str("1m"))]))]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "accepted",
        },
        ValidateTestCase {
            description: "a lag one second under Snowflake's minimum",
            config: maintained(vec![("refresh", map(vec![("lag", Value::Str("59s"))]))]),
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': refresh lag on Snowflake must be at least 1m; got '59s'",
        },
        ValidateTestCase {
            description: "materialized_view on Snowflake",
            config: vec![("materialized", Value::Str("materialized_view"))],
            references: &[],
            query_sql: "select 1",
            expected_outcome: "model 'orders_daily': Snowflake has no materialized_view materialization; use materialized maintained, which builds a dynamic table",
        },
    ];

    for test_case in test_cases {
        let outcome =
            validation_outcome(test_case.config, test_case.references, test_case.query_sql);

        assert_eq!(
            outcome, test_case.expected_outcome,
            "{}",
            test_case.description
        );
    }
}
