//! Public records of recall.

use crate::recall::_helpers::clickhouse_calls as calls;
use crate::recall::_helpers::clickhouse_calls::ConnectionRole;
use crate::recall::_helpers::clickhouse_store::ClickHouseStore;
use crate::recall::_helpers::clickhouse_store::StoreState;
use crate::recall::_helpers::clickhouse_store::connect_store;
use crate::recall::_helpers::clickhouse_store::integer;
use crate::recall::_helpers::clickhouse_store::literal;
use crate::recall::_helpers::clickhouse_store::observed_version;
use crate::recall::_helpers::clickhouse_store::text;
use crate::recall::_helpers::replacements::LifecycleReplacement;
use crate::recall::_helpers::replacements::OutputSink;
use crate::recall::_helpers::replacements::PendingEvent;
use crate::recall::_helpers::replacements::discovered;
use crate::recall::_helpers::sinks::RunningSql;
use crate::recall::_helpers::sinks::lifecycle_event_object;
use crate::recall::_helpers::sinks::recording;
use crate::recall::_helpers::writer::Target;
use crate::recall::constants::INVOCATION_COMPLETED;
use crate::recall::constants::INVOCATION_FAILED;
use crate::recall::constants::INVOCATION_STARTED;
use crate::recall::constants::READ_ONLY_QUERIES;
use crate::recall::constants::READONLY_SETTING;
use crate::recall::constants::STATEMENT_COMPLETED;
use crate::recall::constants::STATEMENT_FAILED;
use crate::recall::constants::UNKNOWN_TABLE_ERROR;
use crate::recall::main::escaped_text::escaped_text;
use crate::recall::main::publish_lifecycle_event::publish_lifecycle_event;
use pyo3::exceptions::PyTypeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::prelude::pyclass;
use pyo3::prelude::pymethods;
use pyo3::types::PyBytes;
use pyo3::types::PyBytesMethods;
use pyo3::types::PyDict;
use pyo3::types::PyString;
use serde_json::Map;
use serde_json::Value;
use sqlbuild_config::models::ConfigValue;
use sqlbuild_core::event_export::main::route_lifecycle_event::route_lifecycle_event;
use sqlbuild_core::event_export::models::ExporterCounts;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::event_exporting::main::export_policy::export_policy;
use sqlbuild_python_base::event_exporting::models::BoundEventExporter;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::project_snapshot::models::WorkerBuild;
use sqlbuild_recall::records::models::OutputStream;
use sqlbuild_recall::records::models::RecallValue;
use sqlbuild_recall::schema::constants::ALL_TABLES;
use sqlbuild_recall::schema::constants::VIEW_NAMES;
use sqlbuild_recall::schema::main::clickhouse_relation::clickhouse_relation;
use sqlbuild_recall::schema::main::create_database_statement::create_database_statement;
use sqlbuild_recall::schema::main::ddl_statements::ddl_statements;
use sqlbuild_recall::schema::main::table_spec::table_spec;
use sqlbuild_recall::schema::main::upgrade_statements::upgrade_statements;
use sqlbuild_recall::schema::models::DdlOptions;
use sqlbuild_recall::schema::models::Dialect;
use sqlbuild_recall::schema::models::Table;
use sqlbuild_recall::settings::models::AuditEvidence;
use sqlbuild_recall::settings::models::RecallSettings;
use sqlbuild_recall::store::constants::LAST_INVOCATION_SELECTOR;
use sqlbuild_recall::store::constants::LISTED_INVOCATION_CANDIDATES;
use sqlbuild_recall::store::errors::StoreError;
use sqlbuild_recall::store::errors::StoreErrorKind;
use sqlbuild_recall::store::main::chosen_invocation::chosen_invocation;
use sqlbuild_recall::store::models::DescribedColumn;
use sqlbuild_recall::store::models::DescribedRelation;
use sqlbuild_recall::store::models::OutputTranscript;
use sqlbuild_recall::store::models::QueryResult;
use sqlbuild_recall::store::types::RecallStore;
use sqlbuild_recall::writer::models::DeliveryStream;
use sqlbuild_recall::writer::models::RecallSession;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

/// What `sqb recall query` reports about a ClickHouse query besides its rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueryStatistics {
    pub rows_read: Option<u64>,
    pub bytes_read: Option<u64>,
    pub query_id: Option<String>,
}

/// Where a ClickHouse store lives and how long it keeps history.
#[derive(Clone, Debug)]
pub struct ClickHouseLocation {
    pub config: Vec<(String, ConfigValue)>,
    pub options: DdlOptions,
}

impl ClickHouseLocation {
    pub fn new(
        config: &[(String, ConfigValue)],
        database: &str,
        retention: u32,
        producer_version: &str,
    ) -> Self {
        Self {
            config: config.to_vec(),
            options: DdlOptions {
                database: Some(database.to_owned()),
                retention_days: retention,
                cluster: None,
                producer_version: producer_version.to_owned(),
            },
        }
    }

    pub(crate) fn connect_action(&self) -> String {
        format!("connect to the Recall store in `{}`", self.database())
    }

    pub(crate) fn database(&self) -> &str {
        self.options.database.as_deref().unwrap_or_default()
    }

    pub(crate) fn table(&self, name: &str) -> String {
        clickhouse_relation(name, &self.options)
    }
}

/// A ClickHouse store opened for `sqb recall`.
#[derive(Debug)]
pub struct ClickHouseReader {
    pub(crate) store: ClickHouseStore,
}

impl ClickHouseReader {
    pub fn open(py: Python<'_>, location: &ClickHouseLocation) -> Result<Self, StoreError> {
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
    pub fn require_store(&self, py: Python<'_>) -> Result<(), StoreError> {
        let state: StoreState = self.store.state(py)?;
        if state.tables.len() < ALL_TABLES.len() || state.views.len() < VIEW_NAMES.len() {
            return Err(StoreError {
                kind: StoreErrorKind::NotFound,
                message: self.not_created_message(),
            });
        }
        Ok(())
    }

    pub(crate) fn not_created_message(&self) -> String {
        format!(
            "no Recall history is recorded yet in ClickHouse database `{}`, or its tables are \
             still being created; run a command first",
            self.database()
        )
    }

    /// The statements that create the store in `location`, or bring it up to this build's schema, read with `SELECT` only.
    pub fn upgrade_statements(
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
    pub fn database(&self) -> &str {
        self.store.location.database()
    }

    /// At most `limit` rows of a read-only query, with the server's statistics. A failure on a store that is not created yet is a store failure (C971), not an invalid query. An unknown relation on a complete store is retried once, since its views may have been created since the query ran; a relation still unknown is the query's own error (C974).
    pub fn query(
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
    pub fn describe(&self, py: Python<'_>) -> Result<Vec<DescribedRelation>, StoreError> {
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
    pub fn resolve_invocation(&self, py: Python<'_>, selector: &str) -> Result<String, StoreError> {
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
    pub fn read_output_transcript(
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

    pub fn close(mut self) {
        self.store.close();
    }
}

/// The replacement sinks of one invocation, configured once discovery has run.
#[derive(Default)]
pub struct RecallReplacements {
    pub(crate) lifecycle: OnceLock<Arc<LifecycleReplacement>>,
}

impl std::fmt::Debug for RecallReplacements {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecallReplacements")
            .field("lifecycle", &self.lifecycle.get().is_some())
            .finish()
    }
}

impl RecallReplacements {
    /// Install the discovered replacements, each `(sink, writer_parameter)` or `None`; whether any filters Recall history.
    pub fn configure(
        &self,
        session: &Arc<RecallSession>,
        (lifecycle, output): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    ) -> PyResult<bool> {
        let mut active: bool = false;
        if let Some((sink, writer_parameter)) = discovered(lifecycle)? {
            let bound = BoundEventExporter::create(
                native_field(&sink, "name")?.extract()?,
                (
                    native_field(&sink, "function")?.unbind(),
                    PyDict::new(sink.py()).into_any().unbind(),
                ),
                (
                    Some(&native_field(&sink, "event_kinds")?),
                    Some(&native_field(&sink, "min_severity")?),
                ),
            )?;
            drop(self.lifecycle.set(Arc::new(LifecycleReplacement {
                function: bound.function,
                writer_parameter,
                filter: bound.filter,
            })));
            active = true;
        }
        if let Some((sink, writer_parameter)) = discovered(output)? {
            let mut streams: [bool; 2] = [false; 2];
            for stream in native_field(&sink, "streams")?.try_iter()? {
                let name: String = native_field(&stream?, "value")?.extract()?;
                if let Some(stream) = OutputStream::from_name(&name) {
                    streams[usize::from(stream == OutputStream::Stderr)] = true;
                }
            }
            session.set_output_replacement(Arc::new(OutputSink {
                function: native_field(&sink, "function")?.unbind(),
                writer_parameter,
                streams,
                invocation_id: session.invocation_id(),
            }));
            active = true;
        }
        Ok(active)
    }

    /// Queue `event` for the lifecycle replacement, which runs on the delivery thread; `false` when there is none.
    pub fn deliver_lifecycle(
        &self,
        py: Python<'_>,
        session: &Arc<RecallSession>,
        event: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        let Some(replacement) = self.lifecycle.get() else {
            return Ok(false);
        };
        let event_type: String = native_field(event, "event_type")?.extract()?;
        let eligible: bool = export_policy(py, &event_type).is_ok_and(|policy| {
            let (_, eligible) = route_lifecycle_event(
                &policy,
                std::slice::from_ref(&replacement.filter),
                vec![ExporterCounts::default()],
            );
            !eligible.is_empty()
        });
        if !eligible {
            session.record_filtered(DeliveryStream::Events, 1);
            return Ok(true);
        }
        let schema_version: i64 = native_field(event, "schema_version")?.extract()?;
        session.publish_pending_event(
            Box::new(PendingEvent {
                replacement: Arc::clone(replacement),
                event: event.clone().unbind(),
            }),
            (&event_type, schema_version),
        );
        Ok(true)
    }
}

/// What the invocation records: its session, when enabled, and the resolved target and adapter.
#[derive(Clone, Debug, Default)]
pub struct InvocationRecall {
    /// The verified worker remains owned until Recall has drained and its claim can be released.
    pub worker: Arc<Mutex<Option<Arc<WorkerBuild>>>>,
    pub session: Option<Arc<RecallSession>>,
    pub target: Option<String>,
    pub adapter: Option<String>,
    /// The command name, for the closing status line.
    pub command: String,
    /// The resolved settings and project directory, for notices at the end of the run.
    pub settings: Option<(RecallSettings, PathBuf)>,
    /// The Recall replacement sinks, configured once runtime extensions are discovered.
    pub replacements: Arc<RecallReplacements>,
    /// A Recall configuration mistake the command fails with before it runs.
    pub configuration_error: Option<String>,
    /// The invocation's own lifecycle events the session has recorded.
    pub invocation_events: Arc<InvocationEvents>,
    /// Which failing audits collect evidence rows, as `recall.audit_evidence` sets it.
    pub audit_evidence: AuditEvidence,
}

/// Which of an invocation's own lifecycle events the sink has recorded, so an interrupted run records each exactly once.
#[derive(Debug, Default)]
pub struct InvocationEvents {
    pub(crate) started: AtomicBool,
    pub(crate) ended: AtomicBool,
}

impl InvocationEvents {
    /// Whether `invocation_started` was recorded.
    pub fn started(&self) -> bool {
        self.started.load(Ordering::SeqCst)
    }

    /// Whether `invocation_completed` or `invocation_failed` was recorded.
    pub fn ended(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }

    pub(crate) fn note(&self, event_type: &str) {
        if event_type == INVOCATION_STARTED {
            self.started.store(true, Ordering::SeqCst);
        } else if event_type == INVOCATION_COMPLETED || event_type == INVOCATION_FAILED {
            self.ended.store(true, Ordering::SeqCst);
        }
    }
}

/// Publishes lifecycle events, through the Recall replacement sink on the delivery thread when one is configured, and SQL artifacts of the active session; delivery is best effort. The SQL of a statement that fails or is cancelled is queued again with its terminal event's priority.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct RecallLifecycleSink {
    pub session: Arc<RecallSession>,
    pub replacements: Arc<RecallReplacements>,
    pub running_sql: Arc<Mutex<RunningSql>>,
    pub invocation_events: Arc<InvocationEvents>,
}

#[pymethods]
impl RecallLifecycleSink {
    /// Record `event`; an event Recall cannot encode is skipped, never raised into the command.
    pub(crate) fn __call__(&self, py: Python<'_>, event: &Bound<'_, PyAny>) -> PyResult<()> {
        recording(py, self.publish(py, event))
    }

    /// Record one SQL artifact; text Recall cannot encode is skipped, never raised into the command.
    #[pyo3(signature = (*, sql, statement_id=None, resource_id=None))]
    pub(crate) fn record_sql(
        &self,
        py: Python<'_>,
        sql: &Bound<'_, PyString>,
        statement_id: Option<&Bound<'_, PyString>>,
        resource_id: Option<&Bound<'_, PyString>>,
    ) -> PyResult<()> {
        let texts = (|| -> PyResult<(String, Option<String>, Option<String>)> {
            Ok((
                escaped_text(sql)?,
                statement_id.map(escaped_text).transpose()?,
                resource_id.map(escaped_text).transpose()?,
            ))
        })();
        recording(
            py,
            texts.map(|(sql, statement_id, resource_id)| {
                self.session
                    .publish_sql(&sql, statement_id.as_deref(), resource_id.as_deref());
                if let Some(statement_id) = statement_id {
                    self.running().insert(statement_id, (sql, resource_id));
                }
            }),
        )
    }
}

impl RecallLifecycleSink {
    pub(crate) fn running(&self) -> std::sync::MutexGuard<'_, RunningSql> {
        self.running_sql
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Forget a statement's SQL at its terminal event; a failed one's SQL is queued again first.
    pub(crate) fn settle_statement(
        &self,
        event: &Bound<'_, PyAny>,
        event_type: &str,
    ) -> PyResult<()> {
        if event_type != STATEMENT_COMPLETED && event_type != STATEMENT_FAILED {
            return Ok(());
        }
        let Some(statement_id) = event.getattr_opt("statement_id")? else {
            return Ok(());
        };
        if statement_id.is_none() {
            return Ok(());
        }
        let statement_id: String = escaped_text(&statement_id.str()?)?;
        let Some((sql, resource_id)) = self.running().remove(&statement_id) else {
            return Ok(());
        };
        if event_type == STATEMENT_FAILED {
            self.session
                .publish_failed_sql(&sql, Some(&statement_id), resource_id.as_deref());
        }
        Ok(())
    }

    pub(crate) fn publish(&self, py: Python<'_>, event: &Bound<'_, PyAny>) -> PyResult<()> {
        let event_type: String = match event.getattr_opt("event_type")? {
            Some(event_type) => event_type.str()?.to_string(),
            None => String::new(),
        };
        self.settle_statement(event, &event_type)?;
        if !self
            .replacements
            .deliver_lifecycle(py, &self.session, event)?
        {
            publish_lifecycle_event(py, &self.session, event)?;
        }
        self.invocation_events.note(&event_type);
        Ok(())
    }
}

/// Publishes the records a Recall replacement sink keeps; valid only while the sink call runs.
#[pyclass(module = "sqlbuild._native", name = "RecallWriter", frozen)]
#[derive(Debug)]
pub struct RecallWriter {
    pub(crate) target: Target,
    pub(crate) open: AtomicBool,
}

impl RecallWriter {
    /// A writer for one lifecycle event handed to the lifecycle replacement.
    pub fn lifecycle() -> Self {
        Self::new(Target::Lifecycle(Mutex::new(Vec::new())))
    }

    /// A writer for the output chunk with `sequence` handed to the output replacement.
    pub fn output(sequence: u64) -> Self {
        Self::new(Target::Output {
            sequence,
            published: Mutex::new(None),
        })
    }

    pub(crate) fn new(target: Target) -> Self {
        Self {
            target,
            open: AtomicBool::new(true),
        }
    }

    /// End the sink call: later publishing raises, and the events or output bytes published are returned.
    pub fn finish(&self) -> (Vec<Map<String, Value>>, Option<Vec<u8>>) {
        self.open.store(false, Ordering::SeqCst);
        match &self.target {
            Target::Lifecycle(published) => (
                std::mem::take(&mut *published.lock().unwrap_or_else(PoisonError::into_inner)),
                None,
            ),
            Target::Output { published, .. } => (
                Vec::new(),
                published
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .take(),
            ),
        }
    }

    pub(crate) fn ensure_open(&self) -> PyResult<()> {
        if self.open.load(Ordering::SeqCst) {
            return Ok(());
        }
        Err(PyValueError::new_err(
            "a RecallWriter can publish only while the sink call it was passed to runs",
        ))
    }
}

#[pymethods]
impl RecallWriter {
    /// Record `event` in Recall; for the lifecycle replacement sink.
    pub(crate) fn publish_lifecycle(
        &self,
        py: Python<'_>,
        event: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.ensure_open()?;
        let Target::Lifecycle(published) = &self.target else {
            return Err(PyValueError::new_err(
                "this RecallWriter belongs to a command-output sink; use publish_command_output",
            ));
        };
        if !event.is_instance(&host_entry(py, ExecutorHost::LifecycleEvent)?)? {
            return Err(PyTypeError::new_err(
                "publish_lifecycle expects a LifecycleEvent",
            ));
        }
        if let Some(object) = lifecycle_event_object(py, event)? {
            published
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(object);
        }
        Ok(())
    }

    /// Record `record.data` in Recall as the output chunk the sink was passed; at most once per call.
    pub(crate) fn publish_command_output(
        &self,
        py: Python<'_>,
        record: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.ensure_open()?;
        let Target::Output {
            sequence,
            published,
        } = &self.target
        else {
            return Err(PyValueError::new_err(
                "this RecallWriter belongs to a lifecycle-event sink; use publish_lifecycle",
            ));
        };
        if !record.is_instance(&host_entry(py, ExecutorHost::CommandOutputRecord)?)? {
            return Err(PyTypeError::new_err(
                "publish_command_output expects a CommandOutputRecord",
            ));
        }
        let record_sequence: u64 = native_field(record, "sequence")?.extract()?;
        if record_sequence != *sequence {
            return Err(PyValueError::new_err(format!(
                "publish_command_output expects the record passed to this call (sequence \
                 {sequence}) or a copy made with record.replace(), not sequence {record_sequence}"
            )));
        }
        let data: Vec<u8> = native_field(record, "data")?
            .cast::<PyBytes>()
            .map_err(|_| PyTypeError::new_err("record.data must be bytes"))?
            .as_bytes()
            .to_vec();
        let mut slot = published.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_some() {
            return Err(PyValueError::new_err(
                "a command-output record can be published once per sink call",
            ));
        }
        *slot = Some(data);
        Ok(())
    }
}
