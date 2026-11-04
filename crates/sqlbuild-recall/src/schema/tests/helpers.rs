use std::collections::{BTreeMap, BTreeSet};

use crate::schema::constants::{ALL_TABLES, VIEW_NAMES};
use crate::schema::main::ddl_script::ddl_script;
use crate::schema::main::sqlite_retention_sql::sqlite_retention_sql;
use crate::schema::main::table_columns::table_columns;
use crate::schema::main::table_statements::table_statements;
use crate::schema::main::view_columns::view_columns;
use crate::schema::models::{DdlOptions, Dialect, ObservedSchema, Table};

/// Options rendering into `observability` with 14-day retention, optionally on `cluster`.
pub(crate) fn options(cluster: Option<&str>) -> DdlOptions {
    DdlOptions {
        database: Some("observability".to_owned()),
        retention_days: 14,
        cluster: cluster.map(str::to_owned),
        producer_version: "9.8.7".to_owned(),
    }
}

/// The ClickHouse `CREATE TABLE` statement for `table`.
pub(crate) fn clickhouse_table(table: &str, cluster: Option<&str>) -> String {
    let quoted = format!("`{table}`");
    table_statements(Dialect::ClickHouse, &options(cluster))
        .into_iter()
        .find(|statement| statement.contains(&quoted))
        .unwrap_or_default()
}

/// The `fragments` that `statement` contains, in order.
pub(crate) fn contained<'a>(statement: &str, fragments: &[&'a str]) -> Vec<&'a str> {
    fragments
        .iter()
        .copied()
        .filter(|fragment| statement.contains(fragment))
        .collect()
}

/// Schema objects after running the SQLite script `runs` times, and the statements that fail.
pub(crate) fn sqlite_schema_after(runs: usize) -> (usize, Vec<String>) {
    let connection = rusqlite::Connection::open_in_memory().expect("an in-memory database opens");
    let script = ddl_script(Dialect::Sqlite, &options(None));
    let mut failures: Vec<String> = Vec::new();
    for run in 0..runs {
        let outcome: rusqlite::Result<()> = connection.execute_batch(&script);
        failures.extend(outcome.err().map(|error| format!("run {run}: {error}")));
    }
    let retention = sqlite_retention_sql();
    let mut statements: Vec<String> = VIEW_NAMES
        .iter()
        .map(|view| format!("SELECT * FROM \"{view}\""))
        .collect();
    statements.push(retention.expired_invocations.clone());
    statements.extend(
        retention
            .delete_invocations
            .iter()
            .map(|(_, delete)| delete.clone()),
    );
    statements.push(retention.delete_orphan_artifacts.clone());
    for statement in statements {
        let prepared: rusqlite::Result<rusqlite::Statement<'_>> = connection.prepare(&statement);
        failures.extend(prepared.err().map(|error| format!("{statement}: {error}")));
    }
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type IN ('table', 'view')",
            [],
            |row| row.get(0),
        )
        .expect("the count query runs");
    (usize::try_from(count).unwrap_or_default(), failures)
}

/// A store at `version` holding every table, column and view of this build, less `without_column` and `without_view`.
pub(crate) fn observed_store(
    version: Option<i64>,
    without_column: (Table, &str),
    without_view: &str,
) -> ObservedSchema {
    ObservedSchema {
        tables: ALL_TABLES
            .iter()
            .map(|table| (*table, present_columns(*table, without_column)))
            .collect(),
        views: VIEW_NAMES
            .iter()
            .filter(|name| **name != without_view)
            .map(|name| (*name).to_owned())
            .collect(),
        version,
    }
}

fn present_columns(table: Table, (without_table, without_name): (Table, &str)) -> BTreeSet<String> {
    table_columns(table)
        .into_iter()
        .filter(|column| (table, column.name) != (without_table, without_name))
        .map(|column| column.name.to_owned())
        .collect()
}

/// This build's view columns, keyed by owned names as a store reports them.
pub(crate) fn current_views() -> BTreeMap<String, BTreeSet<String>> {
    view_columns()
        .expect("this build's views can be listed")
        .into_iter()
        .map(|(view, columns)| (view.to_owned(), columns))
        .collect()
}

/// This build's view columns with `added` added to and `removed` taken from `statements`.
pub(crate) fn changed_statements_view(
    added: &[&str],
    removed: &[&str],
) -> BTreeMap<String, BTreeSet<String>> {
    let mut views = current_views();
    let statements = views.get_mut("statements").expect("statements is a view");
    statements.extend(added.iter().map(|column| (*column).to_owned()));
    statements.retain(|column| !removed.contains(&column.as_str()));
    views
}

/// Views that pass a table through with `SELECT *`, and that table.
const PASSTHROUGH_VIEWS: [(&str, Table); 3] = [
    ("events", Table::Events),
    ("diagnostics", Table::Diagnostics),
    ("conflicts", Table::Conflicts),
];

/// The columns a ClickHouse view body exposes: its passed-through table's, or the aliases of its final top-level `SELECT` list.
pub(crate) fn clickhouse_view_columns(view: &str, body: &str) -> BTreeSet<String> {
    PASSTHROUGH_VIEWS
        .iter()
        .find(|(name, _)| *name == view)
        .map(|(_, table)| {
            table_columns(*table)
                .into_iter()
                .map(|column| column.name.to_owned())
                .collect()
        })
        .unwrap_or_else(|| select_list_names(body))
}

fn select_list_names(body: &str) -> BTreeSet<String> {
    let start: usize = body.rfind("\nSELECT").map_or(0, |index| index + 1);
    let list: &str = body[start + "SELECT".len()..]
        .split("\nFROM ")
        .next()
        .unwrap_or_default();
    top_level_items(list)
        .iter()
        .map(|item| item_name(item))
        .collect()
}

fn top_level_items(list: &str) -> Vec<String> {
    let marked: String = list
        .chars()
        .scan((0_i32, false), |(depth, quoted), character| {
            let separator: bool = *depth == 0 && !*quoted && character == ',';
            *quoted ^= character == '\'';
            *depth += i32::from(!*quoted && character == '(');
            *depth -= i32::from(!*quoted && character == ')');
            Some([character, '\u{0}'][usize::from(separator)])
        })
        .collect();
    marked.split('\u{0}').map(str::to_owned).collect()
}

fn item_name(item: &str) -> String {
    let normalized: String = item.split_whitespace().collect::<Vec<&str>>().join(" ");
    normalized.rsplit_once(" AS ").map_or_else(
        || normalized.rsplit('.').next().unwrap_or_default().to_owned(),
        |(_, alias)| alias.to_owned(),
    )
}
