//! Row readers and a killable writer process shared by the control-store schema contracts.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::credentials::tests::location_helpers::create_location;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::open_store::open_store;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::tests::schema::test_types::StoreDialect;
use crate::store::tests::store_helpers::run;
use crate::store::tests::store_test_types::Backend;
use crate::store::types::Row;
use crate::store_migrations::constants::{POSTGRES_BASELINE_CHECKSUMS, SQLITE_BASELINE_CHECKSUMS};
use crate::store_migrations::models::StoreMigration;

const CHILD_STORE_VARIABLE: &str = "SQLBUILD_TEST_CRASH_STORE";
const ACKNOWLEDGED: &str = "committed ";

pub(crate) fn text(row: &Row, key: &str) -> String {
    row[key].as_str().expect("text column").to_owned()
}

/// In a `kill_writer_after` process, commit one location at a time and acknowledge each, forever.
pub(crate) fn write_until_killed_when_spawned() {
    let Ok(path) = std::env::var(CHILD_STORE_VARIABLE) else {
        return;
    };
    run(async {
        let store = open_store(&path).await.expect("open store");
        migrate_store(&store).await.expect("migrate store");
        for index in 0..u64::MAX {
            create_location(&store, &format!("orders-{index}"), "test-admin")
                .await
                .expect("create location");
            println!("{ACKNOWLEDGED}{index}");
        }
    });
}

/// Kill a writer running `test` after `commits` acknowledgements; return them and the WAL bytes.
pub(crate) fn kill_writer_after(test: &str, path: &Path, commits: usize) -> (usize, u64) {
    let name = format!(
        "{}::{test}",
        module_path!()
            .split_once("::")
            .expect("crate path")
            .1
            .rsplit_once("::")
            .expect("schema module")
            .0
    );
    let mut child = Command::new(std::env::current_exe().expect("test binary"))
        .args([name.as_str(), "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_STORE_VARIABLE, path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start writer process");
    let acknowledged = BufReader::new(child.stdout.take().expect("writer output"))
        .lines()
        .map(|line| line.expect("writer line"))
        .filter(|line| line.starts_with(ACKNOWLEDGED))
        .take(commits)
        .count();
    child.kill().expect("kill writer");
    child.wait().expect("reap writer");
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    let wal_bytes = std::fs::metadata(wal).map_or(0, |metadata| metadata.len());
    (acknowledged, wal_bytes)
}

/// The number of locations a reopened store holds.
pub(crate) fn surviving_locations(path: &Path) -> usize {
    run(async {
        let store = open_store(path.to_str().expect("UTF-8 path"))
            .await
            .expect("reopen store");
        let transaction = begin(&store, TransactionMode::Read)
            .await
            .expect("begin read");
        let (transaction, rows) = query(
            transaction,
            "SELECT COUNT(*) AS locations FROM sqb_locations",
            vec![],
        )
        .await
        .expect("count locations");
        commit(transaction).await.expect("finish read");
        usize::try_from(rows[0]["locations"].as_i64().expect("count")).expect("count")
    })
}

/// Commit `commits` rows of `bytes` each, one transaction at a time with none overlapping; return the WAL bytes.
pub(crate) fn wal_bytes_after_sequential_writes(path: &Path, commits: usize, bytes: usize) -> u64 {
    run(async {
        let store = open_store(path.to_str().expect("UTF-8 path"))
            .await
            .expect("open store");
        let transaction = begin(&store, TransactionMode::Write).await.expect("begin");
        let (transaction, _) = query(transaction, "CREATE TABLE wal_probe (payload TEXT)", vec![])
            .await
            .expect("create probe table");
        commit(transaction).await.expect("commit table");
        for index in 0..commits {
            let transaction = begin(&store, TransactionMode::Write).await.expect("begin");
            let (transaction, _) = query(
                transaction,
                "INSERT INTO wal_probe (payload) VALUES ($1)",
                vec![Parameter::Text(format!("{index:0>bytes$}"))],
            )
            .await
            .expect("write probe row");
            commit(transaction).await.expect("commit probe row");
        }
        let mut wal = path.as_os_str().to_owned();
        wal.push("-wal");
        std::fs::metadata(wal).map_or(0, |metadata| metadata.len())
    })
}

const BASELINE_SQLITE: &str = include_str!("baseline/sqlite.sql");
const BASELINE_POSTGRES: &str = include_str!("baseline/postgres.sql");
pub(crate) const BASELINE_ROWS: &str = include_str!("baseline/rows.sql");
const SHAPE_SQLITE: &str = "SELECT type || ' ' || name || ': ' || COALESCE(sql, '') AS item \
    FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND tbl_name <> 'sqb_schema_version'";
const SHAPE_POSTGRES: &str = "SELECT 'column ' || table_name || '.' || column_name || ' ' || data_type \
    || ' ' || is_nullable || ' ' || COALESCE(column_default, '') AS item \
    FROM information_schema.columns WHERE table_schema = current_schema() \
    AND table_name <> 'sqb_schema_version' \
    UNION ALL SELECT 'constraint ' || conrelid::regclass::text || ' ' || conname || ' ' \
    || pg_get_constraintdef(oid) FROM pg_constraint \
    WHERE connamespace = current_schema()::regnamespace \
    AND conrelid::regclass::text <> 'sqb_schema_version' \
    UNION ALL SELECT 'index ' || indexname || ' ' || replace(indexdef, current_schema() || '.', '') \
    FROM pg_indexes \
    WHERE schemaname = current_schema() AND tablename <> 'sqb_schema_version' \
    UNION ALL SELECT 'view ' || viewname || ' ' || definition FROM pg_views \
    WHERE schemaname = current_schema()";
/// The SQLite facts the migration contracts use.
pub(crate) const SQLITE_STORE: StoreDialect = StoreDialect {
    backend: Backend::Sqlite,
    baseline_schema: BASELINE_SQLITE,
    baseline_checksum: SQLITE_BASELINE_CHECKSUMS[0],
    time_type: "TEXT",
    shape_sql: SHAPE_SQLITE,
    recreate_command: "rm -f '",
};
/// The PostgreSQL facts the migration contracts use.
pub(crate) const POSTGRES_STORE: StoreDialect = StoreDialect {
    backend: Backend::Postgres,
    baseline_schema: BASELINE_POSTGRES,
    baseline_checksum: POSTGRES_BASELINE_CHECKSUMS[0],
    time_type: "TIMESTAMPTZ",
    shape_sql: SHAPE_POSTGRES,
    recreate_command: "dropdb -h '",
};
/// The tables `rows.sql` fills: installation, locations, people, roles, tokens, triggers, run nodes and runs.
pub(crate) const POPULATED_TABLES: [&str; 10] = [
    "sqb_installation",
    "sqb_locations",
    "sqb_snapshots",
    "sqb_principals",
    "sqb_role_assignments",
    "sqb_access_tokens",
    "sqb_triggers",
    "sqb_trigger_state",
    "sqb_run_nodes",
    "sqb_runs",
];

/// Run `statements` (separated by blank lines) in one write transaction.
async fn execute_all(store: &ControlStore, statements: &[&str]) {
    let mut transaction = begin(store, TransactionMode::Write)
        .await
        .expect("begin write");
    for statement in statements {
        (transaction, _) = query(transaction, statement, vec![])
            .await
            .unwrap_or_else(|error| panic!("{statement}: {error}"));
    }
    commit(transaction).await.expect("commit");
}

fn blank_line_statements(sql: &str) -> Vec<&str> {
    sql.split(";\n\n")
        .map(|statement| statement.trim().trim_end_matches(';'))
        .filter(|statement| !statement.is_empty())
        .collect()
}

/// A store as a release before schema versions left it: `dialect`'s baseline schema and checksum, and `rows.sql`.
pub(crate) async fn create_unversioned_baseline(
    store: &ControlStore,
    dialect: &StoreDialect,
    checksum: &str,
) {
    let version_table = format!(
        "CREATE TABLE sqb_schema_version (singleton BIGINT PRIMARY KEY CHECK (singleton = 1), \
         checksum TEXT NOT NULL, applied_at {} NOT NULL)",
        dialect.time_type
    );
    let stamp = format!(
        "INSERT INTO sqb_schema_version (singleton, checksum, applied_at) \
         VALUES (1, '{checksum}', '2026-11-01T08:00:00Z')"
    );
    let installation = "INSERT INTO sqb_installation (singleton, installation_id, created_at) \
         VALUES (1, '018f0000-0000-7000-8000-000000000000', '2026-11-01T08:00:00Z')";
    let (schema, rows) = (
        dialect.baseline_schema.replace("\r\n", "\n"),
        BASELINE_ROWS.replace("\r\n", "\n"),
    );
    let mut statements: Vec<&str> = vec![version_table.as_str()];
    statements.extend(blank_line_statements(&schema));
    statements.extend([installation, stamp.as_str()]);
    statements.extend(blank_line_statements(&rows));
    execute_all(store, &statements).await;
}

/// Run `sql`'s `;`-separated statements in one write transaction.
pub(crate) async fn execute(store: &ControlStore, sql: &str) {
    let statements: Vec<&str> = sql
        .split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .collect();
    execute_all(store, &statements).await;
}

/// The store's tables, columns, constraints, indexes and views, normalised so equal schemas compare equal.
pub(crate) async fn schema_shape(store: &ControlStore, dialect: &StoreDialect) -> Vec<String> {
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("begin read");
    let (transaction, rows) = query(transaction, dialect.shape_sql, vec![])
        .await
        .expect("read schema");
    commit(transaction).await.expect("finish read");
    let mut shape: Vec<String> = rows
        .iter()
        .map(|row| {
            let item: String = text(row, "item").replace('"', "");
            let words: Vec<&str> = item.split_whitespace().collect();
            words
                .join(" ")
                .replace("( ", "(")
                .replace(" )", ")")
                .replace(" ,", ",")
        })
        .collect();
    shape.sort();
    shape
}

/// Every row of `tables`, in primary-key order.
pub(crate) async fn table_rows(store: &ControlStore, tables: &[&str]) -> Vec<Vec<Row>> {
    let mut transaction = begin(store, TransactionMode::Read)
        .await
        .expect("begin read");
    let mut all: Vec<Vec<Row>> = Vec::new();
    for table in tables {
        let rows: Vec<Row>;
        (transaction, rows) = query(
            transaction,
            &format!("SELECT * FROM {table} ORDER BY 1"),
            vec![],
        )
        .await
        .expect("read table");
        all.push(rows);
    }
    commit(transaction).await.expect("finish read");
    all
}

/// `rows` with only the columns each table's first `before` row has, so columns a migration added don't count as changes.
pub(crate) fn without_added_columns(rows: Vec<Vec<Row>>, before: &[Vec<Row>]) -> Vec<Vec<Row>> {
    rows.into_iter()
        .zip(before)
        .map(|(table, old)| {
            let Some(columns) = old.first() else {
                return table;
            };
            table
                .into_iter()
                .map(|mut row| {
                    row.retain(|column, _| columns.contains_key(column));
                    row
                })
                .collect()
        })
        .collect()
}

/// The text `item` column of every row `sql` returns, in its order.
pub(crate) async fn query_items(store: &ControlStore, sql: &str) -> Vec<String> {
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("begin read");
    let (transaction, rows) = query(transaction, sql, vec![])
        .await
        .unwrap_or_else(|error| panic!("{sql}: {error}"));
    commit(transaction).await.expect("finish read");
    rows.iter().map(|row| text(row, "item")).collect()
}

/// The store's recorded schema version.
pub(crate) async fn recorded_version(store: &ControlStore) -> Option<i64> {
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("begin read");
    let (transaction, rows) = query(transaction, "SELECT * FROM sqb_schema_version", vec![])
        .await
        .expect("read version");
    commit(transaction).await.expect("finish read");
    rows.first()
        .and_then(|row| row.get("version"))
        .and_then(serde_json::Value::as_i64)
}

/// SQLite statements rebuilding `sqb_runs` with `skipped` added to its status check, and its indexes; empty on PostgreSQL.
pub(crate) async fn rebuilt_runs_table(store: &ControlStore) -> &'static str {
    let ControlStore::Sqlite(_) = store else {
        return "";
    };
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("begin read");
    let (transaction, rows) = query(
        transaction,
        "SELECT type, sql FROM sqlite_master WHERE tbl_name = 'sqb_runs' AND sql IS NOT NULL \
         ORDER BY type DESC",
        vec![],
    )
    .await
    .expect("read sqb_runs definition");
    commit(transaction).await.expect("finish read");
    let mut definitions = rows.iter().map(|row| text(row, "sql"));
    let table = definitions
        .next()
        .expect("sqb_runs table")
        .replacen("CREATE TABLE sqb_runs", "CREATE TABLE sqb_runs_rebuilt", 1)
        .replace("'cancelled'))", "'cancelled', 'skipped'))");
    let indexes: Vec<String> = definitions.collect();
    let sql = format!(
        "{table};\nINSERT INTO sqb_runs_rebuilt SELECT * FROM sqb_runs;\nDROP TABLE sqb_runs;\n\
         ALTER TABLE sqb_runs_rebuilt RENAME TO sqb_runs;\n{}",
        indexes.join(";\n")
    );
    Box::leak(sql.into_boxed_str())
}

/// The numbered names a migration plan lists for `migrations` after version `from`.
pub(crate) fn migration_names(migrations: &[StoreMigration], from: i64) -> Vec<String> {
    migrations
        .iter()
        .skip(usize::try_from(from - 1).expect("version at or after the baseline"))
        .zip(from + 1..)
        .map(|(migration, number)| format!("{number:04}_{}", migration.name))
        .collect()
}
