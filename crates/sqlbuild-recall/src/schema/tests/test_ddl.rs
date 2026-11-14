use crate::schema::constants::{ALL_TABLES, RECALL_SCHEMA_VERSION, VIEW_NAMES};
use crate::schema::main::add_column_statement::add_column_statement;
use crate::schema::main::ddl_script::ddl_script;
use crate::schema::main::table_spec::table_spec;
use crate::schema::main::table_statements::table_statements;
use crate::schema::main::upgrade_statements::upgrade_statements;
use crate::schema::main::view_statements::view_statements;
use crate::schema::models::{ColumnSpec, ColumnType, Dialect, ObservedSchema, Table};
use crate::schema::tests::helpers::{
    clickhouse_table, contained, observed_store, options, sqlite_schema_after,
};
use crate::schema::tests::test_types::{
    AddColumnTestCase, FragmentTestCase, SqliteScriptTestCase, StatementSetTestCase,
    UpgradeStatementsTestCase,
};

const NEW_COLUMN: ColumnSpec = ColumnSpec {
    name: "new_field",
    column_type: ColumnType::Integer,
    nullable: false,
};
/// Views whose rows or counts would repeat with a key's stored copies.
const FINAL_VIEWS: [&str; 8] = [
    "audit_results",
    "output_invocations",
    "output_tail",
    "output_transcripts",
    "events",
    "diagnostics",
    "artifacts",
    "conflicts",
];
/// Views that fold every copy of a key to the same result.
const FOLDING_VIEWS: [&str; 2] = ["invocations", "statements"];

#[test]
fn given_sqlite_script_when_run_then_every_table_view_and_retention_statement_prepares() {
    let test_cases = [
        SqliteScriptTestCase {
            description: "a fresh database gets every object",
            runs: 1,
            expected_objects: ALL_TABLES.len() + VIEW_NAMES.len(),
            expected_failures: Vec::new(),
        },
        SqliteScriptTestCase {
            description: "rerunning the script is idempotent",
            runs: 2,
            expected_objects: ALL_TABLES.len() + VIEW_NAMES.len(),
            expected_failures: Vec::new(),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            sqlite_schema_after(test_case.runs),
            (test_case.expected_objects, test_case.expected_failures),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_clickhouse_tables_when_rendered_then_engines_ttl_and_partitions_follow_the_table() {
    let test_cases = [
        FragmentTestCase {
            description: "output is a replacing table partitioned and expired by start time",
            statement: clickhouse_table("recall_output", None),
            expected_fragments: &[
                "CREATE TABLE IF NOT EXISTS `observability`.`recall_output` (",
                "`data` String",
                "`priority` Nullable(String)",
                "ENGINE = ReplacingMergeTree()",
                "PARTITION BY toDate(invocation_started_at)",
                "ORDER BY (invocation_id, sequence)",
                "TTL invocation_started_at + INTERVAL 14 DAY",
            ],
        },
        FragmentTestCase {
            description: "artifacts keep the latest sighting for an extra week",
            statement: clickhouse_table("recall_artifacts", None),
            expected_fragments: &[
                "ENGINE = ReplacingMergeTree(seen_at)",
                "TTL seen_at + INTERVAL 21 DAY",
            ],
        },
        FragmentTestCase {
            description: "heartbeats collapse repeated deliveries of a beat",
            statement: clickhouse_table("recall_heartbeats", None),
            expected_fragments: &[
                "ENGINE = ReplacingMergeTree()",
                "ORDER BY (invocation_id, beat_at)",
            ],
        },
        FragmentTestCase {
            description: "schema records are append-only",
            statement: clickhouse_table("recall_meta", None),
            expected_fragments: &["ENGINE = MergeTree()"],
        },
        FragmentTestCase {
            description: "clustered replacing tables are replicated",
            statement: clickhouse_table("recall_output", Some("analytics")),
            expected_fragments: &[
                " ON CLUSTER `analytics` (",
                "ReplicatedReplacingMergeTree('/clickhouse/tables/{uuid}/{shard}', '{replica}')",
            ],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            contained(&test_case.statement, test_case.expected_fragments),
            test_case.expected_fragments.to_vec(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_clickhouse_statement_sets_when_rendered_then_every_statement_has_the_shared_shape() {
    let views = |cluster: Option<&str>| -> Vec<String> {
        view_statements(Dialect::ClickHouse, &options(cluster))
            .into_iter()
            .map(|(_, statement)| statement)
            .collect()
    };
    let named_views = |names: &[&str]| -> Vec<String> {
        view_statements(Dialect::ClickHouse, &options(None))
            .into_iter()
            .filter(|(name, _)| names.contains(name))
            .map(|(_, statement)| statement)
            .collect()
    };
    let test_cases = [
        StatementSetTestCase {
            description: "every clustered table is created on the cluster",
            statements: table_statements(Dialect::ClickHouse, &options(Some("analytics"))),
            required_fragments: &[" ON CLUSTER `analytics` ("],
            expected_count: ALL_TABLES.len(),
        },
        StatementSetTestCase {
            description: "clustered views are replaced on the cluster",
            statements: views(Some("analytics")),
            required_fragments: &["CREATE OR REPLACE VIEW", "ON CLUSTER `analytics`"],
            expected_count: VIEW_NAMES.len(),
        },
        StatementSetTestCase {
            description: "the administrator's script ends by recording the schema version and the producing release",
            statements: ddl_script(Dialect::ClickHouse, &options(None))
                .rsplit(";\n\n")
                .take(1)
                .map(str::to_owned)
                .collect(),
            required_fragments: &[
                "INSERT INTO `observability`.`recall_meta`",
                "SELECT 'schema', 'schema_version', '6'",
                "6, '9.8.7', now64(6)",
            ],
            expected_count: 1,
        },
        StatementSetTestCase {
            description: "SQL text stays visible as long as its TTL keeps it",
            statements: views(None)
                .into_iter()
                .filter(|statement| statement.contains("`recall_artifacts` FINAL"))
                .collect(),
            required_fragments: &[
                "`recall_artifacts` FINAL WHERE seen_at >= now64(6) - INTERVAL 21 DAY",
            ],
            expected_count: 1,
        },
        StatementSetTestCase {
            description: "views filter by retention",
            statements: views(None),
            required_fragments: &["now64(6) - INTERVAL 14 DAY"],
            expected_count: VIEW_NAMES.len(),
        },
        StatementSetTestCase {
            description: "views that list or count rows read each key once",
            statements: named_views(&FINAL_VIEWS),
            required_fragments: &[" FINAL WHERE "],
            expected_count: FINAL_VIEWS.len(),
        },
        StatementSetTestCase {
            description: "views that only fold a key's copies read the tables without FINAL",
            statements: named_views(&FOLDING_VIEWS)
                .into_iter()
                .filter(|statement| !statement.contains(" FINAL "))
                .collect(),
            required_fragments: &["GROUP BY invocation_id, invocation_started_at"],
            expected_count: FOLDING_VIEWS.len(),
        },
        StatementSetTestCase {
            description: "no view sets query settings or depends on join_use_nulls",
            statements: views(None)
                .into_iter()
                .filter(|statement| !statement.contains("SETTINGS"))
                .collect(),
            required_fragments: &["CREATE OR REPLACE VIEW"],
            expected_count: VIEW_NAMES.len(),
        },
    ];

    for test_case in test_cases {
        let shaped: usize = test_case
            .statements
            .iter()
            .filter(|statement| {
                contained(statement, test_case.required_fragments).len()
                    == test_case.required_fragments.len()
            })
            .count();
        assert_eq!(
            (test_case.statements.len(), shaped),
            (test_case.expected_count, test_case.expected_count),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_new_column_when_upgraded_then_the_statement_is_additive_and_nullable() {
    let spec = table_spec(Table::Events);
    let test_cases = [
        AddColumnTestCase {
            description: "SQLite columns are nullable by default",
            statement: add_column_statement(Dialect::Sqlite, &options(None), (spec, &NEW_COLUMN)),
            expected_statement: "ALTER TABLE \"recall_events\" ADD COLUMN \"new_field\" INTEGER",
        },
        AddColumnTestCase {
            description: "ClickHouse columns are added idempotently as Nullable",
            statement: add_column_statement(
                Dialect::ClickHouse,
                &options(None),
                (spec, &NEW_COLUMN),
            ),
            expected_statement: "ALTER TABLE `observability`.`recall_events` ADD COLUMN IF NOT \
                                 EXISTS `new_field` Nullable(Int64)",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            test_case.statement, test_case.expected_statement,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_an_observed_store_when_upgraded_then_only_the_missing_additive_statements_are_listed() {
    let test_cases = [
        UpgradeStatementsTestCase {
            description: "a current store needs nothing",
            observed: observed_store(Some(RECALL_SCHEMA_VERSION), (Table::Meta, ""), ""),
            dialect: Dialect::Sqlite,
            expected_count: 0,
            expected_prefixes: &[],
        },
        UpgradeStatementsTestCase {
            description: "an empty store gets the whole creation script",
            observed: ObservedSchema::default(),
            dialect: Dialect::ClickHouse,
            expected_count: ALL_TABLES.len() + VIEW_NAMES.len() + 1,
            expected_prefixes: &["CREATE TABLE IF NOT EXISTS `observability`.`recall_meta`"],
        },
        UpgradeStatementsTestCase {
            description: "a store a version behind adds its column, replaces views and records the version",
            observed: observed_store(
                Some(RECALL_SCHEMA_VERSION - 1),
                (Table::Heartbeats, "subcommand"),
                "",
            ),
            dialect: Dialect::ClickHouse,
            expected_count: 1 + VIEW_NAMES.len() + 1,
            expected_prefixes: &[
                "ALTER TABLE `observability`.`recall_heartbeats` ADD COLUMN IF NOT EXISTS `subcommand` Nullable(String)",
                "CREATE OR REPLACE VIEW `observability`.`invocations`",
                "INSERT INTO `observability`.`recall_meta`",
            ],
        },
        UpgradeStatementsTestCase {
            description: "a SQLite store without a recorded version replaces its views",
            observed: observed_store(None, (Table::Meta, ""), ""),
            dialect: Dialect::Sqlite,
            expected_count: 2 * VIEW_NAMES.len() + 1,
            expected_prefixes: &[
                "DROP VIEW IF EXISTS \"invocations\"",
                "INSERT OR IGNORE INTO",
            ],
        },
        UpgradeStatementsTestCase {
            description: "a current store missing a view only creates it",
            observed: observed_store(Some(RECALL_SCHEMA_VERSION), (Table::Meta, ""), "statements"),
            dialect: Dialect::ClickHouse,
            expected_count: 1,
            expected_prefixes: &["CREATE VIEW IF NOT EXISTS `observability`.`statements`"],
        },
    ];

    for test_case in test_cases {
        let statements: Vec<String> =
            upgrade_statements(test_case.dialect, &options(None), &test_case.observed);
        let found: Vec<&str> = test_case
            .expected_prefixes
            .iter()
            .copied()
            .filter(|prefix| {
                statements
                    .iter()
                    .any(|statement| statement.starts_with(prefix))
            })
            .collect();
        assert_eq!(
            (statements.len(), found.as_slice()),
            (test_case.expected_count, test_case.expected_prefixes),
            "{}",
            test_case.description
        );
    }
}
