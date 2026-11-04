//! The ClickHouse Recall store: written and read through the ClickHouse adapter's connection.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use pyo3::prelude::Python;
use sqlbuild_config::models::ConfigValue;
use sqlbuild_recall::clickhouse::constants::{BATCH_ROWS, BATCH_WINDOW_MS};
use sqlbuild_recall::clickhouse::main::deduplication_token::deduplication_token;
use sqlbuild_recall::clickhouse::main::encode_rows::encode_rows;
use sqlbuild_recall::clickhouse::main::parse_column_encoding::parse_column_encoding;
use sqlbuild_recall::clickhouse::main::plan_insert::plan_insert;
use sqlbuild_recall::clickhouse::main::store_state_sql::store_state_sql;
use sqlbuild_recall::clickhouse::main::stored_rows_sql::stored_rows_sql;
use sqlbuild_recall::clickhouse::main::ttl_mismatches::ttl_mismatches;
use sqlbuild_recall::clickhouse::models::{StoredColumn, StoredRow, TtlMismatch};
use sqlbuild_recall::records::main::epoch_micros::epoch_micros;
use sqlbuild_recall::records::main::format_timestamp::format_timestamp;
use sqlbuild_recall::records::main::meta_row::meta_row;
use sqlbuild_recall::records::models::{OutputStream, RecallRow, RecallValue, RowEnvelope};
use sqlbuild_recall::schema::constants::{
    ALL_TABLES, ARTIFACT_REFRESH_DAYS, META_SCHEMA_COMPONENT, META_SCHEMA_VERSION_KEY,
    RECALL_SCHEMA_VERSION, VIEW_NAMES,
};
use sqlbuild_recall::schema::main::add_column_statement::add_column_statement;
use sqlbuild_recall::schema::main::clickhouse_relation::clickhouse_relation;
use sqlbuild_recall::schema::main::create_database_statement::create_database_statement;
use sqlbuild_recall::schema::main::ddl_statements::ddl_statements;
use sqlbuild_recall::schema::main::render_script::render_script;
use sqlbuild_recall::schema::main::retention_fix_statements::retention_fix_statements;
use sqlbuild_recall::schema::main::schema_age::schema_age;
use sqlbuild_recall::schema::main::store_version_with_views::store_version_with_views;
use sqlbuild_recall::schema::main::table_columns::table_columns;
use sqlbuild_recall::schema::main::table_spec::table_spec;
use sqlbuild_recall::schema::main::table_statements::table_statements;
use sqlbuild_recall::schema::main::upgrade_statements::upgrade_statements;
use sqlbuild_recall::schema::main::version_plan::version_plan;
use sqlbuild_recall::schema::main::view_columns::view_columns;
use sqlbuild_recall::schema::main::view_statements::view_statements;
use sqlbuild_recall::schema::models::{
    ColumnSpec, DdlOptions, Dialect, ObservedSchema, SchemaAge, StoreVersion, Table, VersionPlan,
};
use sqlbuild_recall::settings::constants::CLICKHOUSE_DATABASE_KEY;
use sqlbuild_recall::store::constants::{LAST_INVOCATION_SELECTOR, LISTED_INVOCATION_CANDIDATES};
use sqlbuild_recall::store::errors::{StoreError, StoreErrorKind};
use sqlbuild_recall::store::main::chosen_invocation::chosen_invocation;
use sqlbuild_recall::store::models::{
    DescribedColumn, DescribedRelation, InsertOutcome, OutputTranscript, QueryResult,
};
use sqlbuild_recall::store::types::{RecallStore, RecallStoreOpener};

use crate::bindings::_helpers::recall::clickhouse_calls::{
    self as calls, ConnectionRole, QueryStatistics, StoreHandle,
};
use crate::bindings::_helpers::recall::constants::{
    READ_ONLY_QUERIES, READONLY_SETTING, SECONDS_PER_DAY, SERVER_DEFAULT_DATABASE,
    UNKNOWN_TABLE_ERROR,
};

/// Where a ClickHouse store lives and how long it keeps history.
#[derive(Clone, Debug)]
pub(crate) struct ClickHouseLocation {
    pub(crate) config: Vec<(String, ConfigValue)>,
    pub(crate) options: DdlOptions,
}

impl ClickHouseLocation {
    pub(crate) fn new(config: &[(String, ConfigValue)], database: &str, retention: u32) -> Self {
        Self {
            config: config.to_vec(),
            options: DdlOptions {
                database: Some(database.to_owned()),
                retention_days: retention,
                cluster: None,
                producer_version: env!("CARGO_PKG_VERSION").to_owned(),
            },
        }
    }

    fn connect_action(&self) -> String {
        format!("connect to the Recall store in `{}`", self.database())
    }

    fn database(&self) -> &str {
        self.options.database.as_deref().unwrap_or_default()
    }

    fn table(&self, name: &str) -> String {
        clickhouse_relation(name, &self.options)
    }
}

/// Opens the store on the delivery thread; `retention_notice` decides whether a retention conflict is reported this time.
#[derive(Debug)]
pub(crate) struct ClickHouseStoreOpener {
    pub(crate) location: ClickHouseLocation,
    pub(crate) retention_notice: fn(&Path, &str) -> bool,
    pub(crate) project_dir: PathBuf,
    /// What opening did that nothing is wrong with, such as creating the database, kept until the session takes it even when a later step fails.
    pub(crate) information: Mutex<Vec<String>>,
}

/// The Recall relations present: tables with their `CREATE` queries, views, and the stored schema version.
struct StoreState {
    tables: Vec<(String, String)>,
    views: HashSet<String>,
    /// The views' `CREATE` queries, which show what an older version wrote when columns cannot.
    view_definitions: Vec<String>,
    version: i64,
    /// Seconds since the newest version row was recorded.
    version_age_seconds: i64,
}

/// A store ready to write: its tables with their `CREATE` queries, and the upgrade an administrator must run when this user could not.
struct ReadyStore {
    tables: Vec<(String, String)>,
    behind: Vec<String>,
}

/// The store the delivery thread writes.
struct ClickHouseStore {
    location: ClickHouseLocation,
    connection: Option<StoreHandle>,
    columns: HashMap<Table, Vec<StoredColumn>>,
    notices: Vec<String>,
    information: Vec<String>,
}

fn text(value: Option<&RecallValue>) -> String {
    match value {
        Some(RecallValue::Text(text)) => text.clone(),
        Some(RecallValue::Blob(data)) => String::from_utf8_lossy(data).into_owned(),
        Some(RecallValue::Integer(number)) => number.to_string(),
        _ => String::new(),
    }
}

fn integer(value: Option<&RecallValue>) -> i64 {
    match value {
        Some(RecallValue::Integer(number)) => *number,
        _ => 0,
    }
}

/// `text` as a ClickHouse string literal.
fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}

impl ClickHouseStore {
    fn connection(&self) -> Result<StoreHandle, StoreError> {
        self.connection.ok_or_else(|| StoreError {
            kind: StoreErrorKind::Other,
            message: "the Recall store connection is closed".to_owned(),
        })
    }

    fn query(&self, py: Python<'_>, sql: &str, action: &str) -> Result<QueryResult, StoreError> {
        calls::query(py, self.connection()?, (sql, &[], None), action).map(|(result, _)| result)
    }

    fn command(&self, py: Python<'_>, sql: &str, action: &str) -> Result<(), StoreError> {
        calls::command(py, self.connection()?, sql, action)
    }

    /// The Recall relations present, with their `CREATE` queries, and the stored schema version.
    fn state(&self, py: Python<'_>) -> Result<StoreState, StoreError> {
        let sql = store_state_sql(self.location.database());
        let relations: QueryResult = self.query(py, &sql.relations, "inspect the Recall store")?;
        let mut tables: Vec<(String, String)> = Vec::new();
        let mut views: HashSet<String> = HashSet::new();
        let mut view_definitions: Vec<String> = Vec::new();
        for row in &relations.rows {
            let name = text(row.first());
            if integer(row.get(1)) == 1 {
                views.insert(name);
                view_definitions.push(text(row.get(2)));
            } else {
                tables.push((name, text(row.get(2))));
            }
        }
        let (version, version_age_seconds): (i64, i64) = if tables
            .iter()
            .any(|(name, _)| name == table_spec(Table::Meta).name)
        {
            let found = self.query(py, &sql.schema_version, "read the Recall schema version")?;
            let row = found.rows.first();
            (
                integer(row.and_then(|row| row.first())),
                integer(row.and_then(|row| row.get(1))),
            )
        } else {
            (0, 0)
        };
        Ok(StoreState {
            tables,
            views,
            view_definitions,
            version,
            version_age_seconds,
        })
    }

    /// The recorded version, or without one how the tables, read by `read_columns`, and the views compare with this build's; either is lowered when a view definition shows an older version wrote it.
    fn store_version(
        &self,
        py: Python<'_>,
        state: &StoreState,
    ) -> Result<StoreVersion, StoreError> {
        let definitions: Vec<&str> = state.view_definitions.iter().map(String::as_str).collect();
        Ok(store_version_with_views(
            self.columns_version(py, state)?,
            &definitions,
        ))
    }

    fn columns_version(
        &self,
        py: Python<'_>,
        state: &StoreState,
    ) -> Result<StoreVersion, StoreError> {
        if state.version > 0 {
            return Ok(StoreVersion::Recorded {
                version: state.version,
                age_seconds: state.version_age_seconds,
                views_visible: !state.views.is_empty(),
            });
        }
        let mut expected = view_columns().map_err(|message| StoreError {
            kind: StoreErrorKind::Other,
            message: format!("could not list this build's Recall view columns: {message}"),
        })?;
        let mut observed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for table in ALL_TABLES {
            let name: &'static str = table_spec(table).name;
            expected.insert(
                name,
                table_columns(table)
                    .iter()
                    .map(|column| column.name.to_owned())
                    .collect(),
            );
            let stored = self.columns.get(&table).map_or(&[][..], Vec::as_slice);
            observed.insert(
                name.to_owned(),
                stored.iter().map(|column| column.name.clone()).collect(),
            );
        }
        let sql = store_state_sql(self.location.database());
        let found = self.query(py, &sql.view_columns, "inspect the Recall views")?;
        for row in &found.rows {
            observed
                .entry(text(row.first()))
                .or_default()
                .insert(text(row.get(1)));
        }
        Ok(StoreVersion::Unrecorded(schema_age(
            &expected,
            &observed,
            &VIEW_NAMES,
        )))
    }

    fn database_exists(&self, py: Python<'_>) -> Result<bool, StoreError> {
        let found = self.query(
            py,
            &format!(
                "SELECT count() FROM system.databases WHERE name = {}",
                literal(self.location.database())
            ),
            "inspect the Recall store",
        )?;
        Ok(integer(found.rows.first().and_then(|row| row.first())) > 0)
    }

    /// Create the configured database when it is missing, saying so; an existing one, whatever its engine, is left alone. A user who may not create it gets the statement to hand to an administrator.
    fn ensure_database(&mut self, py: Python<'_>) -> Result<(), StoreError> {
        if self.database_exists(py)? {
            return Ok(());
        }
        let Some(statement) = create_database_statement(&self.location.options) else {
            return Ok(());
        };
        match self.command(py, &statement, "create the Recall database") {
            Ok(()) => {
                self.information.push(format!(
                    "created ClickHouse database `{}` for the store",
                    self.location.database()
                ));
                Ok(())
            }
            Err(error) if error.kind == StoreErrorKind::Permission => Err(StoreError {
                kind: StoreErrorKind::Permission,
                message: format!(
                    "{} ({})",
                    missing_database_reason(self.location.database()),
                    error.message
                ),
            }),
            Err(error) => Err(error),
        }
    }

    fn read_columns(&mut self, py: Python<'_>) -> Result<(), StoreError> {
        let sql = store_state_sql(self.location.database());
        let found = self.query(py, &sql.columns, "inspect the Recall tables")?;
        let mut columns: HashMap<Table, Vec<StoredColumn>> = HashMap::new();
        for row in &found.rows {
            let table_name = text(row.first());
            let Some(table) = ALL_TABLES
                .iter()
                .find(|table| table_spec(**table).name == table_name)
            else {
                continue;
            };
            let name = text(row.get(1));
            let type_text = text(row.get(2));
            let (encoding, nullable) = parse_column_encoding(&type_text).ok_or_else(|| StoreError {
                kind: StoreErrorKind::Other,
                message: format!(
                    "Recall column {table_name}.{name} has type {type_text}, which Recall does not \
                     write"
                ),
            })?;
            columns.entry(*table).or_default().push(StoredColumn {
                name,
                encoding,
                nullable,
            });
        }
        self.columns = columns;
        Ok(())
    }

    /// What the store holds, from its read columns, `views` and recorded `version`.
    fn observed(&self, views: &HashSet<String>, version: i64) -> ObservedSchema {
        let mut observed = ObservedSchema {
            views: views.iter().cloned().collect(),
            version: (version > 0).then_some(version),
            ..ObservedSchema::default()
        };
        for (table, columns) in &self.columns {
            let names = observed.tables.entry(*table).or_default();
            for column in columns {
                names.insert(column.name.clone());
            }
        }
        observed
    }

    /// Create a store whose tables are missing. On existing tables add missing columns, create missing views, and replace views on a version upgrade; when retention removed the version row, the views' columns tell whether the store is older, current or newer, and a newer store's views are never replaced; views whose definitions show an older version wrote them make the store older either way. A user who may not run that DDL writes the columns that exist and gets the statements an administrator must run. A version is recorded only once the views are settled, never lower than the store holds, and again before retention removes the newest row.
    fn ensure_schema(
        &mut self,
        py: Python<'_>,
        envelope: &RowEnvelope,
    ) -> Result<ReadyStore, StoreError> {
        let state: StoreState = self.state(py)?;
        if state.tables.len() < ALL_TABLES.len() {
            self.create(py, envelope, &state.views)?;
            return Ok(ReadyStore {
                tables: self.state(py)?.tables,
                behind: Vec::new(),
            });
        }
        self.read_columns(py)?;
        let store_version: StoreVersion = self.store_version(py, &state)?;
        let plan: VersionPlan = version_plan(store_version, self.location.options.retention_days);
        let StoreState { tables, views, .. } = state;
        let before: ObservedSchema = self.observed(&views, observed_version(store_version));
        let behind = |store: &Self| {
            upgrade_statements(Dialect::ClickHouse, &store.location.options, &before)
        };
        if self.add_columns(py)? {
            return Ok(ReadyStore {
                behind: behind(self),
                tables,
            });
        }
        let view_ddl: Vec<(&str, String)> = if plan.view_ddl {
            view_statements(Dialect::ClickHouse, &self.location.options)
        } else {
            Vec::new()
        };
        for (name, statement) in view_ddl {
            let statement: String = if plan.replace_views {
                statement
            } else if !views.contains(name) {
                statement.replacen("CREATE OR REPLACE VIEW", "CREATE VIEW IF NOT EXISTS", 1)
            } else {
                continue;
            };
            match self.command(py, &statement, "create the Recall views") {
                Ok(()) => {}
                Err(error) if error.kind == StoreErrorKind::Permission => {
                    return Ok(ReadyStore {
                        behind: behind(self),
                        tables,
                    });
                }
                Err(error) => return Err(error),
            }
        }
        if let Some(version) = plan.record {
            self.record_version(py, envelope, version)?;
        }
        Ok(ReadyStore {
            tables,
            behind: Vec::new(),
        })
    }

    /// Create every table, column and view of a store whose tables are missing.
    fn create(
        &mut self,
        py: Python<'_>,
        envelope: &RowEnvelope,
        views: &HashSet<String>,
    ) -> Result<(), StoreError> {
        self.ensure_database(py)?;
        for statement in table_statements(Dialect::ClickHouse, &self.location.options) {
            self.command(py, &statement, "create the Recall tables")?;
        }
        self.read_columns(py)?;
        if self.add_columns(py)? {
            return Err(StoreError {
                kind: StoreErrorKind::Permission,
                message: "the Recall tables need columns this user may not add".to_owned(),
            });
        }
        for (name, statement) in view_statements(Dialect::ClickHouse, &self.location.options) {
            if !views.contains(name) {
                self.command(py, &statement, "create the Recall views")?;
            }
        }
        self.record_version(py, envelope, RECALL_SCHEMA_VERSION)
    }

    /// Add the additive columns the stored tables lack; `true` when the user may not.
    fn add_columns(&mut self, py: Python<'_>) -> Result<bool, StoreError> {
        let missing: Vec<(Table, ColumnSpec)> = self.missing_columns();
        let statements: Vec<String> = missing
            .iter()
            .map(|(table, column)| {
                add_column_statement(
                    Dialect::ClickHouse,
                    &self.location.options,
                    (table_spec(*table), column),
                )
            })
            .collect();
        for statement in &statements {
            match self.command(py, statement, "add a Recall column") {
                Ok(()) => {}
                Err(error) if error.kind == StoreErrorKind::Permission => {
                    self.read_columns(py)?;
                    return Ok(true);
                }
                Err(error) => return Err(error),
            }
        }
        if !missing.is_empty() {
            self.read_columns(py)?;
        }
        Ok(false)
    }

    /// Every column this build writes that the stored tables lack.
    fn missing_columns(&self) -> Vec<(Table, ColumnSpec)> {
        let mut missing: Vec<(Table, ColumnSpec)> = Vec::new();
        for table in ALL_TABLES {
            let present: &[StoredColumn] = self.columns.get(&table).map_or(&[], Vec::as_slice);
            for column in table_columns(table) {
                if !present.iter().any(|stored| stored.name == column.name) {
                    missing.push((table, column));
                }
            }
        }
        missing
    }

    /// Record this build's schema version with an insert, which a writer allowed only `SELECT` and `INSERT` may run.
    fn record_version(
        &self,
        py: Python<'_>,
        envelope: &RowEnvelope,
        version: i64,
    ) -> Result<(), StoreError> {
        let now = format_timestamp(SystemTime::now());
        let row = meta_row(
            envelope,
            (
                META_SCHEMA_COMPONENT,
                META_SCHEMA_VERSION_KEY,
                &version.to_string(),
            ),
            &now,
        );
        self.write(py, Table::Meta, &[row], SystemTime::now())
    }

    fn write(
        &self,
        py: Python<'_>,
        table: Table,
        rows: &[RecallRow],
        now: SystemTime,
    ) -> Result<(), StoreError> {
        let columns: &[StoredColumn] = self.columns.get(&table).map_or(&[], Vec::as_slice);
        let micros: i64 = epoch_micros(now);
        let (names, block) = encode_rows(rows, columns, micros).map_err(|message| StoreError {
            kind: StoreErrorKind::Other,
            message,
        })?;
        calls::insert(
            py,
            self.connection()?,
            (&self.location.table(table_spec(table).name), &names, &block),
            &deduplication_token(rows),
        )
    }

    fn stored_rows(
        &self,
        py: Python<'_>,
        rows: &[RecallRow],
    ) -> Result<Vec<StoredRow>, StoreError> {
        let Some(sql) = stored_rows_sql(rows, &self.location.options) else {
            return Ok(Vec::new());
        };
        let found = self.query(py, &sql, "read stored Recall rows")?;
        Ok(found
            .rows
            .iter()
            .map(|row| {
                let hashes: Vec<String> = text(row.get(3)).split(',').map(str::to_owned).collect();
                let newest: String = text(row.get(4));
                StoredRow {
                    dataset: text(row.first()),
                    record_key: text(row.get(1)),
                    first_hash: text(row.get(2)),
                    hashes,
                    newest_seen_at: (!newest.is_empty()).then_some(newest),
                }
            })
            .collect())
    }

    /// The notice for a store behind this build that this user may not upgrade: history is written to the columns that exist until an administrator runs `statements`.
    fn behind_notice(&self, statements: &[String]) -> String {
        format!(
            "the Recall store in ClickHouse database `{}` is behind this SQLBuild version, and \
             this user may not upgrade it; history is still written, without what the newer \
             version adds. An administrator should run:\n\n{}\n`sqb recall schema --ddl` \
             prints these statements again.\n",
            self.location.database(),
            render_script(statements)
        )
    }

    fn retention_conflict(&self, tables: &[(String, String)]) -> Option<String> {
        let mismatches: Vec<TtlMismatch> =
            ttl_mismatches(tables, self.location.options.retention_days);
        let first: &TtlMismatch = mismatches.first()?;
        let stored: String = first
            .stored_days
            .map_or_else(|| "no day TTL".to_owned(), |days| format!("{days} days"));
        Some(format!(
            "the ClickHouse store in `{}` keeps history for {stored} by its table TTLs, but this \
             project is configured for {} days; retention is a store-wide policy and Recall never \
             changes TTLs itself. An administrator can apply the configured retention with:\n\n{}",
            self.location.database(),
            self.location.options.retention_days,
            retention_fix_statements(
                &mismatches
                    .iter()
                    .map(|mismatch| mismatch.table.clone())
                    .collect::<Vec<_>>(),
                &self.location.options
            )
        ))
    }
}

impl RecallStore for ClickHouseStore {
    fn insert(&self, rows: &[RecallRow], _give_up: Instant) -> Result<InsertOutcome, StoreError> {
        Python::attach(|py| {
            let now: SystemTime = SystemTime::now();
            let refresh_before: SystemTime = now
                .checked_sub(Duration::from_secs(
                    u64::from(ARTIFACT_REFRESH_DAYS) * SECONDS_PER_DAY,
                ))
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let stored: Vec<StoredRow> = self.stored_rows(py, rows)?;
            let plan = plan_insert(
                rows.to_vec(),
                &stored,
                (&format_timestamp(now), &format_timestamp(refresh_before)),
            );
            let mut outcome: InsertOutcome = plan.outcome;
            for (table, table_rows) in &plan.inserts {
                match self.write(py, *table, table_rows, now) {
                    Ok(()) => {}
                    Err(error) if error.kind == StoreErrorKind::Busy => return Err(error),
                    Err(error) => {
                        outcome.failed.push(*table);
                        outcome.failure.get_or_insert(error.message);
                    }
                }
            }
            let failed: &[Table] = &outcome.failed;
            outcome
                .delivered
                .retain(|(table, _)| !failed.contains(table));
            Ok(outcome)
        })
    }

    fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    fn close(&mut self) {
        if let Some(connection) = self.connection.take() {
            Python::attach(|py| calls::close(py, connection));
        }
    }
}

impl RecallStoreOpener for ClickHouseStoreOpener {
    fn open(
        &self,
        envelope: &RowEnvelope,
        _give_up: Instant,
    ) -> Result<Box<dyn RecallStore>, StoreError> {
        Python::attach(|py| {
            let mut store = ClickHouseStore {
                location: self.location.clone(),
                connection: Some(connect_store(py, &self.location, ConnectionRole::Writer)?),
                columns: HashMap::new(),
                notices: Vec::new(),
                information: Vec::new(),
            };
            let ensured = store.ensure_schema(py, envelope);
            self.information
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .append(&mut store.information);
            match ensured {
                Ok(ReadyStore { tables, behind }) => {
                    if !behind.is_empty()
                        && (self.retention_notice)(
                            &self.project_dir,
                            &format!("{}:upgrade", self.location.database()),
                        )
                    {
                        store.notices.push(store.behind_notice(&behind));
                    }
                    if let Some(notice) = store.retention_conflict(&tables)
                        && (self.retention_notice)(
                            &self.project_dir,
                            &format!("{}:retention", self.location.database()),
                        )
                    {
                        store.notices.push(notice);
                    }
                    Ok(Box::new(store) as Box<dyn RecallStore>)
                }
                Err(error) => {
                    store.close();
                    Err(error)
                }
            }
        })
    }

    fn take_information(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .information
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    fn batch_rows(&self) -> usize {
        BATCH_ROWS
    }

    fn batch_window(&self) -> Duration {
        Duration::from_millis(BATCH_WINDOW_MS)
    }
}

/// A ClickHouse store opened for `sqb recall`.
pub(crate) struct ClickHouseReader {
    store: ClickHouseStore,
}

impl ClickHouseReader {
    pub(crate) fn open(py: Python<'_>, location: &ClickHouseLocation) -> Result<Self, StoreError> {
        let store = ClickHouseStore {
            location: location.clone(),
            connection: Some(calls::connect(
                py,
                (&location.config, ConnectionRole::Reader),
                &location.connect_action(),
            )?),
            columns: HashMap::new(),
            notices: Vec::new(),
            information: Vec::new(),
        };
        Ok(Self { store })
    }

    /// Fail with "no history yet" when the store's tables or views are missing; queries check only after they fail, so a successful one costs no extra round trip.
    pub(crate) fn require_store(&self, py: Python<'_>) -> Result<(), StoreError> {
        let state: StoreState = self.store.state(py)?;
        if state.tables.len() < ALL_TABLES.len() || state.views.len() < VIEW_NAMES.len() {
            return Err(StoreError {
                kind: StoreErrorKind::NotFound,
                message: self.not_created_message(),
            });
        }
        Ok(())
    }

    fn not_created_message(&self) -> String {
        format!(
            "no Recall history is recorded yet in ClickHouse database `{}`, or its tables are \
             still being created; run a command first",
            self.database()
        )
    }

    /// The statements that create the store in `location`, or bring it up to this build's schema, read with `SELECT` only.
    pub(crate) fn upgrade_statements(
        py: Python<'_>,
        location: &ClickHouseLocation,
    ) -> Result<Vec<String>, StoreError> {
        let mut store = ClickHouseStore {
            location: location.clone(),
            connection: Some(connect_store(py, location, ConnectionRole::Reader)?),
            columns: HashMap::new(),
            notices: Vec::new(),
            information: Vec::new(),
        };
        match store.database_exists(py) {
            Ok(true) => {}
            Ok(false) => {
                store.close();
                return Ok(create_database_statement(&location.options)
                    .into_iter()
                    .chain(ddl_statements(Dialect::ClickHouse, &location.options))
                    .collect());
            }
            Err(error) => {
                store.close();
                return Err(error);
            }
        }
        let observed = {
            let state: StoreState = store.state(py)?;
            store.read_columns(py)?;
            let version: i64 = if state.tables.len() < ALL_TABLES.len() {
                state.version
            } else {
                observed_version(store.store_version(py, &state)?)
            };
            Ok(store.observed(&state.views, version))
        };
        store.close();
        Ok(upgrade_statements(
            Dialect::ClickHouse,
            &location.options,
            &observed?,
        ))
    }

    /// The database the store lives in.
    pub(crate) fn database(&self) -> &str {
        self.store.location.database()
    }

    /// At most `limit` rows of a read-only query, with the server's statistics. A failure on a store that is not created yet is a store failure (C971), not an invalid query. An unknown relation on a complete store is retried once, since its views may have been created since the query ran; a relation still unknown is the query's own error (C974).
    pub(crate) fn query(
        &self,
        py: Python<'_>,
        sql: &str,
        limit: Option<usize>,
    ) -> Result<(QueryResult, QueryStatistics), StoreError> {
        let settings: [(&str, i64); 1] = [(READONLY_SETTING, READ_ONLY_QUERIES)];
        let row_cap: Option<usize> = limit.map(|limit| limit.saturating_add(1));
        let run = || {
            calls::query(
                py,
                self.store.connection()?,
                (sql, &settings, row_cap),
                "run the Recall query",
            )
        };
        let rejected = |failure: StoreError| StoreError {
            kind: if failure.kind == StoreErrorKind::Busy {
                StoreErrorKind::Other
            } else {
                StoreErrorKind::Query
            },
            message: failure.message,
        };
        let (mut result, statistics) = match run() {
            Ok(found) => found,
            Err(failure) => match self.require_store(py) {
                Err(missing) if missing.kind == StoreErrorKind::NotFound => {
                    return Err(StoreError {
                        kind: StoreErrorKind::Other,
                        message: missing.message,
                    });
                }
                Ok(()) if failure.message.contains(UNKNOWN_TABLE_ERROR) => {
                    run().map_err(rejected)?
                }
                Ok(()) | Err(_) => return Err(rejected(failure)),
            },
        };
        if let Some(limit) = limit
            && result.rows.len() > limit
        {
            result.rows.truncate(limit);
            result.truncated = true;
        }
        Ok((result, statistics))
    }

    /// The store's views, then its tables, with their columns.
    pub(crate) fn describe(&self, py: Python<'_>) -> Result<Vec<DescribedRelation>, StoreError> {
        let names: Vec<String> = VIEW_NAMES
            .iter()
            .map(|name| literal(name))
            .chain(
                ALL_TABLES
                    .iter()
                    .map(|table| literal(table_spec(*table).name)),
            )
            .collect();
        let found = self.store.query(
            py,
            &format!(
                "SELECT c.table, if(t.engine = 'View', 'view', 'table') AS kind, c.name, c.type \
                 FROM system.columns AS c INNER JOIN system.tables AS t \
                 ON t.database = c.database AND t.name = c.table \
                 WHERE c.database = {} AND c.table IN ({}) \
                 ORDER BY kind = 'table', c.table, c.position",
                literal(self.database()),
                names.join(", ")
            ),
            "describe the Recall store",
        )?;
        let mut relations: Vec<DescribedRelation> = Vec::new();
        for row in &found.rows {
            let name = text(row.first());
            if relations.last().is_none_or(|last| last.name != name) {
                relations.push(DescribedRelation {
                    name,
                    kind: text(row.get(1)),
                    columns: Vec::new(),
                });
            }
            let declared_type = text(row.get(3));
            if let Some(relation) = relations.last_mut() {
                relation.columns.push(DescribedColumn {
                    name: text(row.get(2)),
                    nullable: declared_type.starts_with("Nullable("),
                    declared_type,
                });
            }
        }
        Ok(relations)
    }

    /// The full invocation ID an ID, unique prefix or `last` names.
    pub(crate) fn resolve_invocation(
        &self,
        py: Python<'_>,
        selector: &str,
    ) -> Result<String, StoreError> {
        let location = &self.store.location;
        let started = format!(
            "(SELECT invocation_id, invocation_started_at FROM {} \
             UNION ALL SELECT invocation_id, invocation_started_at FROM {})",
            location.table(table_spec(Table::Invocations).name),
            location.table(table_spec(Table::Heartbeats).name),
        );
        let sql = if selector == LAST_INVOCATION_SELECTOR {
            format!(
                "SELECT invocation_id FROM {started} \
                 ORDER BY invocation_started_at DESC, invocation_id DESC LIMIT 1"
            )
        } else {
            format!(
                "SELECT DISTINCT invocation_id FROM {started} \
                 WHERE startsWith(invocation_id, {}) ORDER BY invocation_id LIMIT {}",
                literal(selector),
                LISTED_INVOCATION_CANDIDATES + 1
            )
        };
        let found = self.store.query(py, &sql, "find the invocation")?;
        let candidates: Vec<String> = found.rows.iter().map(|row| text(row.first())).collect();
        chosen_invocation(selector, &candidates)
    }

    /// One invocation's exact output bytes, deduplicated by sequence, and its completeness.
    pub(crate) fn read_output_transcript(
        &self,
        py: Python<'_>,
        invocation_id: &str,
        stream: Option<OutputStream>,
    ) -> Result<OutputTranscript, StoreError> {
        let id = literal(invocation_id);
        let stream_filter: String = stream.map_or_else(String::new, |stream| {
            format!(" AND stream = {}", literal(stream.as_str()))
        });
        let output = self.store.location.table(table_spec(Table::Output).name);
        let chunks = self.store.query(
            py,
            &format!(
                "SELECT data FROM (SELECT sequence, stream, data FROM {output} \
                 WHERE invocation_id = {id} ORDER BY sequence, ingested_at LIMIT 1 BY sequence) \
                 WHERE 1{stream_filter} ORDER BY sequence"
            ),
            "read Recall output",
        )?;
        let mut transcript = OutputTranscript {
            invocation_id: invocation_id.to_owned(),
            ..OutputTranscript::default()
        };
        for row in &chunks.rows {
            match row.first() {
                Some(RecallValue::Blob(data)) => transcript.data.extend_from_slice(data),
                Some(RecallValue::Text(text)) => transcript.data.extend_from_slice(text.as_bytes()),
                _ => {}
            }
        }
        let database = self.store.location.table("invocations");
        let status = self.store.query(
            py,
            &format!("SELECT status FROM {database} WHERE invocation_id = {id}"),
            "read the Recall invocation",
        )?;
        transcript.status = status.rows.first().map(|row| text(row.first()));
        let completeness_view = self.store.location.table("output_invocations");
        let completeness = self.store.query(
            py,
            &format!(
                "SELECT missing_sequences, reported_drops, filtered, flush_complete, complete, \
                 gap_reason FROM {completeness_view} WHERE invocation_id = {id}"
            ),
            "read Recall output completeness",
        )?;
        if let Some(row) = completeness.rows.first() {
            transcript.missing_sequences = integer(row.first());
            transcript.reported_drops = integer(row.get(1));
            transcript.filtered = integer(row.get(2));
            transcript.flush_complete = integer(row.get(3)) != 0;
            transcript.complete = integer(row.get(4)) != 0;
            let reason = text(row.get(5));
            transcript.gap_reason = (!reason.is_empty()).then_some(reason);
        }
        Ok(transcript)
    }

    pub(crate) fn close(mut self) {
        self.store.close();
    }
}

/// Why a store cannot be created when its database is missing and the writer may not create it.
pub(crate) fn missing_database_reason(database: &str) -> String {
    format!("ClickHouse database `{database}` does not exist, and this user may not create it")
}

/// Connect for `role`. When the configured database does not exist yet, connect without it, so that it can be created or reported; every statement names the database itself.
fn connect_store(
    py: Python<'_>,
    location: &ClickHouseLocation,
    role: ConnectionRole,
) -> Result<StoreHandle, StoreError> {
    match calls::connect(py, (&location.config, role), &location.connect_action()) {
        Err(error) if error.kind == StoreErrorKind::NotFound => {
            let config: Vec<(String, ConfigValue)> = location
                .config
                .iter()
                .map(|(key, value)| {
                    if key == CLICKHOUSE_DATABASE_KEY {
                        (
                            key.clone(),
                            ConfigValue::String(SERVER_DEFAULT_DATABASE.to_owned()),
                        )
                    } else {
                        (key.clone(), value.clone())
                    }
                })
                .collect();
            calls::connect(py, (&config, role), &location.connect_action())
        }
        connected => connected,
    }
}

/// The version `upgrade_statements` compares with: a store without a version row whose views are current or newer needs no view replacement.
fn observed_version(store: StoreVersion) -> i64 {
    match store {
        StoreVersion::Recorded { version, .. } => version,
        StoreVersion::Unrecorded(SchemaAge::Older) => 0,
        StoreVersion::Unrecorded(SchemaAge::Current | SchemaAge::Newer | SchemaAge::Unseen) => {
            RECALL_SCHEMA_VERSION
        }
    }
}
