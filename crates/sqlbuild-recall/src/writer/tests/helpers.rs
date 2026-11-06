use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::records::main::invocation_row::invocation_row;
use crate::records::models::{
    DeliveryCounts, InvocationContext, OutputStream, RecallValue, RowEnvelope,
};
use crate::schema::models::Table;
use crate::settings::models::AuditEvidence;
use crate::store::main::open_sqlite_store::open_sqlite_store;
use crate::store::main::query_store::query_store;
use crate::store::main::read_output_transcript::read_output_transcript;
use crate::store::models::OpenMode;
use crate::writer::_helpers::queue::WriterState;
use crate::writer::main::start_recall_session::start_recall_session;
use crate::writer::models::{
    DeliveryStream, DeliverySummary, Priority, SessionConfig, SessionStore, SetupOutcome,
};
use crate::writer::tests::test_types::{
    CopyingReplacement, CreatingStore, OutputBytesStore, PartialStore, PrefixReplacement,
    RecordedSession, RecordingReplacement, SlowFinalRowsStore,
};

/// The envelope of invocation `invocation_id`.
pub(crate) fn envelope(invocation_id: &str) -> RowEnvelope {
    RowEnvelope {
        invocation_id: invocation_id.to_owned(),
        invocation_started_at: "2026-10-19T00:00:00.000000Z".to_owned(),
        producer_version: "test".to_owned(),
    }
}

/// Dropped counts per stream and the streams taken after pushing `pushes` into `capacity`.
pub(crate) fn pushed(
    capacity: usize,
    pushes: &[(DeliveryStream, Priority)],
) -> (Vec<(DeliveryStream, u64)>, Vec<DeliveryStream>) {
    let mut state = WriterState::with_capacity(envelope("inv"), (capacity, usize::MAX));
    for (stream, priority) in pushes {
        state.push(
            vec![invocation_row(
                &envelope("inv"),
                &InvocationContext::default(),
            )],
            *stream,
            *priority,
        );
    }
    let mut streams: Vec<DeliveryStream> = pushes.iter().map(|(stream, _)| *stream).collect();
    streams.dedup();
    let dropped: Vec<(DeliveryStream, u64)> = streams
        .iter()
        .map(|stream| (*stream, state.counts[stream.index()].dropped))
        .collect();
    let taken: Vec<DeliveryStream> = state
        .take_batch(usize::MAX)
        .iter()
        .map(|record| record.stream)
        .collect();
    (dropped, taken)
}

/// `(sequence, stream, byte_offset)` of each output chunk after `writes` and a flush.
pub(crate) fn chunked(writes: &[(OutputStream, Vec<u8>)]) -> Vec<[Option<RecallValue>; 3]> {
    let mut state = WriterState::new(envelope("inv"));
    for (stream, data) in writes {
        state.append_output(*stream, data, SystemTime::now());
    }
    state.flush_output();
    state
        .take_batch(usize::MAX)
        .iter()
        .flat_map(|record| record.rows().iter())
        .map(|row| ["sequence", "stream", "byte_offset"].map(|name| row.value(name).cloned()))
        .collect()
}

/// A session configuration writing invocation `invocation_id` to `path`.
pub(crate) fn config(path: PathBuf, invocation_id: &str) -> SessionConfig {
    SessionConfig {
        store: SessionStore::Sqlite(path),
        envelope: envelope(invocation_id),
        context: InvocationContext {
            command: Some("build".to_owned()),
            pid: Some(42),
            ..InvocationContext::default()
        },
        retention_days: 30,
        shutdown_timeout: Duration::from_secs(5),
        diagnostics_min_severity: "info".to_owned(),
        max_output_bytes: None,
        heartbeat_interval: Duration::from_secs(30),
        setup_budget: Duration::from_secs(2),
        maintenance: true,
        maintenance_owner: "test:42".to_owned(),
        defer_context: false,
        audit_evidence: AuditEvidence::WarnAndError,
    }
}

/// Run a session that writes `output`, one completed event, two diagnostics and one SQL twice.
pub(crate) fn recorded_session(output: &[(OutputStream, Vec<u8>)]) -> RecordedSession {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let path = directory.path().join("logs/recall.sqlite3");
    let session = start_recall_session(config(path.clone(), "inv-1"));
    let setup = session.wait_for_setup(Duration::from_secs(10));
    for (stream, data) in output {
        session.append_output(*stream, data);
    }
    session
        .publish_event(
            r#"{"event_id":"e1","event_type":"invocation_completed","schema_version":2,
            "producer":"sqlbuild","producer_version":"1","occurred_at":"2026-10-19T00:00:01.000000Z",
            "invocation_id":"inv-1","invocation_sequence":1,"payload":{"command":"build","exit_code":0,"duration_ms":3.0}}"#,
        )
        .expect("the event is valid");
    for severity in ["debug", "warning"] {
        session
            .publish_diagnostic(&format!(
                r#"{{"occurred_at":"2026-10-19T00:00:00.500000Z","severity":"{severity}","logger":"l","source":"python","message":"m"}}"#
            ))
            .expect("the diagnostic is valid");
    }
    session.publish_sql("select 1", Some("s1"), None);
    session.publish_sql("select 1", Some("s2"), None);
    let summary = session.close();
    let total = summary.total();
    let output_fully_delivered: bool = summary
        .streams
        .iter()
        .find(|(stream, _)| *stream == DeliveryStream::Output)
        .is_some_and(|(_, counts)| counts.delivered == counts.accepted);
    let store = open_sqlite_store(&path, OpenMode::ReadOnly).expect("the store opens");
    let transcript =
        read_output_transcript(&store, "inv-1", None).expect("the transcript can be read");
    let stdout = read_output_transcript(&store, "inv-1", Some(OutputStream::Stdout))
        .expect("the transcript can be read");
    let facts = query_store(
        &store,
        "SELECT (SELECT status FROM invocations), (SELECT COUNT(*) FROM recall_diagnostics), \
         (SELECT COUNT(*) FROM recall_artifacts), (SELECT COUNT(*) FROM recall_delivery), (SELECT command FROM recall_invocations)",
        None,
    )
    .expect("the query is valid");
    RecordedSession {
        setup,
        lost: (total.dropped, total.failed),
        output_fully_delivered,
        transcript: (transcript.data, transcript.complete),
        stdout: stdout.data,
        facts: facts.rows,
    }
}

/// Whether a store under a read-only directory reports permission at setup and degrades.
#[cfg(unix)]
pub(crate) fn unwritable_session() -> (bool, bool) {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o555))
        .expect("test directory permissions can change");
    let session = start_recall_session(config(
        directory.path().join("logs/recall.sqlite3"),
        "inv-2",
    ));
    let setup = session.wait_for_setup(Duration::from_secs(10));
    session.append_output(OutputStream::Stdout, b"still printed\n");
    let summary = session.close();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o755))
        .expect("test directory permissions can change");
    (
        matches!(setup, Some(SetupOutcome::PermissionDenied { .. })),
        summary.degraded_reason.is_some(),
    )
}

/// Seconds `close` took and whether delivery degraded, while another connection holds the write lock.
pub(crate) fn close_under_held_lock(shutdown: Duration) -> (f64, bool) {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let path = directory.path().join("logs/recall.sqlite3");
    let warm = start_recall_session(config(path.clone(), "warm"));
    drop(warm.close());
    let holder = rusqlite::Connection::open(&path).expect("the store opens");
    holder
        .execute_batch("BEGIN EXCLUSIVE")
        .expect("the write lock is taken");
    let session = start_recall_session(SessionConfig {
        shutdown_timeout: shutdown,
        ..config(path, "held")
    });
    session.append_output(OutputStream::Stdout, b"waiting\n");
    let started = std::time::Instant::now();
    let summary = session.close();
    let elapsed = started.elapsed().as_secs_f64();
    drop(holder);
    (elapsed, summary.degraded_reason.is_some())
}

/// The notices and resulting store state after starting on a file that is not a database.
pub(crate) fn session_on_corrupt_store() -> (usize, bool, Vec<Vec<RecallValue>>) {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let path = directory.path().join("recall.sqlite3");
    std::fs::write(&path, b"this is not a database").expect("test files can be written");
    let session = start_recall_session(config(path.clone(), "fresh"));
    session.append_output(OutputStream::Stdout, b"kept\n");
    let summary = session.close();
    let moved: bool = std::fs::read_dir(directory.path())
        .expect("the directory lists")
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"));
    let store = open_sqlite_store(&path, OpenMode::ReadOnly).expect("the store opens");
    let rows = query_store(&store, "SELECT status FROM invocations", None)
        .expect("the query is valid")
        .rows;
    drop(summary);
    (session.notices().len(), moved, rows)
}

fn stream_counts(summary: &DeliverySummary, stream: DeliveryStream) -> DeliveryCounts {
    summary
        .streams
        .iter()
        .find(|(candidate, _)| *candidate == stream)
        .map(|(_, counts)| *counts)
        .unwrap_or_default()
}

/// Artifact counts and stored content rows after publishing the same SQL of `length` bytes `times` times.
pub(crate) fn artifact_counts(
    length: usize,
    times: usize,
) -> (u64, u64, u64, Vec<Vec<RecallValue>>) {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let path = directory.path().join("recall.sqlite3");
    let session = start_recall_session(config(path.clone(), "sql"));
    let sql: String = "x".repeat(length);
    for index in 0..times {
        session.publish_sql(&sql, Some(&format!("s{index}")), None);
    }
    let counts = stream_counts(&session.close(), DeliveryStream::Artifacts);
    let store = open_sqlite_store(&path, OpenMode::ReadOnly).expect("the store opens");
    let stored = query_store(&store, "SELECT COUNT(*) FROM recall_artifacts", None)
        .expect("the query is valid")
        .rows;
    (counts.accepted, counts.delivered, counts.dropped, stored)
}

/// Event counts after publishing `events`; rows are built on the delivery thread.
pub(crate) fn event_counts(events: &[&str]) -> (u64, u64, u64) {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let session = start_recall_session(config(directory.path().join("recall.sqlite3"), "events"));
    let parsed: Vec<serde_json::Map<String, serde_json::Value>> = events
        .iter()
        .map(|event| serde_json::from_str(event).expect("the fixture is a JSON object"))
        .collect();
    parsed
        .into_iter()
        .for_each(|event| session.publish_event_object(event));
    let counts = stream_counts(&session.close(), DeliveryStream::Events);
    (counts.accepted, counts.delivered, counts.failed)
}

/// Output chunks dropped by a queue of `capacity` records, as counted and as each delivered chunk reports them.
pub(crate) fn output_drops(capacity: usize, chunks: usize) -> (u64, Vec<Option<RecallValue>>) {
    let mut state = WriterState::with_capacity(envelope("inv"), (capacity, usize::MAX));
    for index in 0..chunks {
        let stream = [OutputStream::Stdout, OutputStream::Stderr][index % 2];
        state.append_output(stream, b"x", SystemTime::now());
    }
    let mut taken: Vec<crate::writer::_helpers::queue::Queued> = state.take_batch(usize::MAX);
    state.flush_output();
    taken.extend(state.take_batch(usize::MAX));
    let reported: Vec<Option<RecallValue>> = taken
        .iter()
        .flat_map(|record| record.rows().iter())
        .map(|row| row.value("dropped_before").cloned())
        .collect();
    (
        state.counts[DeliveryStream::Output.index()].dropped,
        reported,
    )
}

/// Every output row after `writes` and close, by sequence: sequence, stream, byte offset, data and omitted bytes and sequences.
pub(crate) fn capped(
    limit: Option<u64>,
    writes: &[(OutputStream, &[u8])],
) -> Vec<[Option<RecallValue>; 6]> {
    let mut state = WriterState::new(envelope("inv")).with_output_limit(limit);
    for (stream, data) in writes {
        state.append_output(*stream, data, SystemTime::now());
    }
    state.flush_tails();
    let mut rows: Vec<[Option<RecallValue>; 6]> = state
        .take_batch(usize::MAX)
        .iter()
        .flat_map(|record| record.rows().iter())
        .map(|row| {
            [
                "sequence",
                "stream",
                "byte_offset",
                "data",
                "omitted_bytes",
                "omitted_sequences",
            ]
            .map(|name| row.value(name).cloned())
        })
        .collect();
    rows.sort_by_key(|row| format!("{:020?}", row[0]));
    rows
}

/// A row as `capped` reports it; `omitted` is set on a truncation gap.
pub(crate) fn chunk_facts(
    (sequence, stream, offset): (i64, &str, i64),
    data: &[u8],
    omitted: Option<(i64, i64)>,
) -> [Option<RecallValue>; 6] {
    [
        Some(RecallValue::Integer(sequence)),
        Some(RecallValue::Text(stream.to_owned())),
        Some(RecallValue::Integer(offset)),
        Some(RecallValue::Blob(data.to_vec())),
        Some(omitted.map_or(RecallValue::Null, |(bytes, _)| RecallValue::Integer(bytes))),
        Some(omitted.map_or(RecallValue::Null, |(_, sequences)| {
            RecallValue::Integer(sequences)
        })),
    ]
}

/// Output chunks stored while a session still runs, when heartbeats fall due faster than batches fill.
pub(crate) fn output_stored_while_running(
    heartbeat: Duration,
    wait: Duration,
) -> Vec<Vec<RecallValue>> {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let path = directory.path().join("logs/recall.sqlite3");
    let session = start_recall_session(SessionConfig {
        heartbeat_interval: heartbeat,
        ..config(path.clone(), "beating")
    });
    session.append_output(OutputStream::Stdout, b"progress\n");
    std::thread::sleep(wait);
    let store = open_sqlite_store(&path, OpenMode::ReadOnly).expect("the store opens");
    let stored = query_store(&store, "SELECT COUNT(*) FROM recall_output", None)
        .expect("the query is valid");
    drop(session.close());
    stored.rows
}

/// The transcript and output accounting a session with a deferred context and `PrefixReplacement` stored for `output`, written `discovery` before the replacement is set.
pub(crate) fn replaced_session(
    output: &[(OutputStream, &[u8])],
    discovery: Duration,
) -> (Vec<u8>, Vec<Vec<RecallValue>>) {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let path = directory.path().join("logs/recall.sqlite3");
    let session = start_recall_session(SessionConfig {
        defer_context: true,
        ..config(path.clone(), "replaced")
    });
    for (stream, data) in output {
        session.append_output(*stream, data);
    }
    std::thread::sleep(discovery);
    session.set_output_replacement(Arc::new(PrefixReplacement));
    session.publish_context(true);
    drop(session.close());
    let store = open_sqlite_store(&path, OpenMode::ReadOnly).expect("the store opens");
    let transcript =
        read_output_transcript(&store, "replaced", None).expect("the transcript can be read");
    let facts = query_store(
        &store,
        "SELECT filtered, failed, missing_sequences, filter_active, gap_reason \
         FROM output_invocations",
        None,
    )
    .expect("the query is valid");
    (transcript.data, facts.rows)
}

/// Output bytes a store with a ten-minute batch window received `wait` after `written_bytes` were printed, and the output records dropped by close.
pub(crate) fn sent_before_the_window(written_bytes: usize, wait: Duration) -> (usize, u64) {
    let store = OutputBytesStore::default();
    let session = start_recall_session(SessionConfig {
        store: SessionStore::Hosted(Arc::new(store.clone())),
        ..config(PathBuf::from("unused"), "burst")
    });
    let line: Vec<u8> = vec![b'x'; 4096];
    for _ in 0..written_bytes / line.len() {
        session.append_output(OutputStream::Stdout, &line);
    }
    std::thread::sleep(wait);
    let sent: usize = *store
        .received
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let summary = session.close();
    (
        sent,
        stream_counts(&summary, DeliveryStream::Output).dropped,
    )
}

/// `(accepted, delivered, failed)` of events and output after one batch whose insert into `failing` fails.
pub(crate) fn partially_inserted(failing: Table) -> [(u64, u64, u64); 2] {
    let session = start_recall_session(SessionConfig {
        store: SessionStore::Hosted(Arc::new(PartialStore { failing })),
        ..config(PathBuf::from("unused"), "partial")
    });
    session.append_output(OutputStream::Stdout, b"printed\n");
    session
        .publish_event(
            r#"{"event_id":"e1","event_type":"invocation_completed","schema_version":2,
            "producer":"sqlbuild","producer_version":"1","occurred_at":"2026-10-19T00:00:01.000000Z",
            "invocation_id":"partial","invocation_sequence":1,"payload":{"command":"build"}}"#,
        )
        .expect("the event is valid");
    let summary = session.close();
    [DeliveryStream::Events, DeliveryStream::Output].map(|stream| {
        let counts = stream_counts(&summary, stream);
        (counts.accepted, counts.delivered, counts.failed)
    })
}

/// What `invocations` shows for a session still waiting on discovery, its context row held, after `wait`.
pub(crate) fn invocation_during_discovery(wait: Duration) -> Vec<Vec<RecallValue>> {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let path = directory.path().join("logs/recall.sqlite3");
    let session = start_recall_session(SessionConfig {
        defer_context: true,
        ..config(path.clone(), "discovering")
    });
    std::thread::sleep(wait);
    let store = open_sqlite_store(&path, OpenMode::ReadOnly).expect("the store opens");
    let shown = query_store(
        &store,
        "SELECT command, status, started_at IS NOT NULL, \
         (SELECT COUNT(*) FROM recall_invocations) FROM invocations",
        None,
    )
    .expect("the query is valid");
    drop(session.close());
    shown.rows
}

/// `(accepted, delivered, filtered, failed)` of events whose replacements publish `copies` each.
pub(crate) fn pending_event_counts(copies: &[Option<usize>]) -> (u64, u64, u64, u64) {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let session = start_recall_session(config(directory.path().join("recall.sqlite3"), "pending"));
    for (index, copies) in copies.iter().enumerate() {
        session.publish_pending_event(
            Box::new(CopyingReplacement {
                index,
                copies: *copies,
            }),
            ("run_started", 2),
        );
    }
    let counts = stream_counts(&session.close(), DeliveryStream::Events);
    (
        counts.accepted,
        counts.delivered,
        counts.filtered,
        counts.failed,
    )
}

/// The heartbeat columns the first Recall release lacks, and the view text that reads them.
const CORE_RELEASE_EDITS: [(&str, &str); 3] = [
    (
        ",\n        MIN(command) AS command, MIN(subcommand) AS subcommand",
        "",
    ),
    (
        "COALESCE(context.command, started.command, beats.command)",
        "COALESCE(context.command, started.command)",
    ),
    (
        "COALESCE(context.subcommand, beats.subcommand) AS subcommand",
        "context.subcommand",
    ),
];

/// A store at schema version 1 as the first Recall release creates it: heartbeats without `command` or `subcommand`.
pub(crate) fn core_release_store(path: &std::path::Path) {
    use crate::schema::main::ddl_script::ddl_script;
    use crate::schema::main::view_statements::view_statements;
    use crate::schema::models::{DdlOptions, Dialect};

    let options = DdlOptions {
        database: None,
        retention_days: 30,
        cluster: None,
        producer_version: "9.8.7".to_owned(),
    };
    std::fs::create_dir_all(path.parent().expect("the store has a directory"))
        .expect("the store directory can be created");
    let connection = rusqlite::Connection::open(path).expect("the store opens");
    connection
        .execute_batch(&ddl_script(Dialect::Sqlite, &options))
        .expect("the script runs");
    let views = view_statements(Dialect::Sqlite, &options);
    for (name, _) in &views {
        connection
            .execute_batch(&format!("DROP VIEW \"{name}\""))
            .expect("the view drops");
    }
    connection
        .execute_batch(
            "ALTER TABLE recall_heartbeats DROP COLUMN command; \
             ALTER TABLE recall_heartbeats DROP COLUMN subcommand; \
             INSERT INTO recall_meta (component, key, value, recorded_at, invocation_id, \
             invocation_started_at, schema_version, producer_version, ingested_at, content_hash) \
             VALUES ('schema', 'schema_version', '1', '2026-10-01T00:00:00.000000Z', 'core', \
             '2026-10-01T00:00:00.000000Z', 1, 'core', '2026-10-01T00:00:00.000000Z', '')",
        )
        .expect("the release schema is restored");
    for (_, statement) in views {
        let released: String = CORE_RELEASE_EDITS
            .iter()
            .fold(statement, |text, (new, old)| text.replace(new, old));
        connection
            .execute_batch(&released)
            .expect("the release view is created");
    }
}

/// Write a heartbeat naming the command straight into the store, before any upgrade.
pub(crate) fn insert_heartbeat(path: &std::path::Path) {
    use crate::records::main::heartbeat_row::heartbeat_row;
    use crate::store::main::insert_rows::insert_rows;

    let store = open_sqlite_store(path, OpenMode::ReadWrite).expect("the store opens");
    let beat = heartbeat_row(
        &envelope("core"),
        ("2026-10-19T00:00:01.000000Z", "2026-10-19T00:01:31.000000Z"),
        (None, Some(1)),
        (Some("build"), None),
    );
    insert_rows(&store, &[beat], "2026-10-19T00:00:01.000000Z").expect("the row is written");
}

/// Record a short run through a session.
pub(crate) fn record_run(path: &std::path::Path) {
    let session = start_recall_session(config(path.to_path_buf(), "core"));
    session.append_output(OutputStream::Stdout, b"built\n");
    drop(session.wait_for_setup(Duration::from_secs(10)));
    std::thread::sleep(Duration::from_millis(300));
    drop(session.close());
}

/// What `write` leaves in a store the first Recall release created.
pub(crate) fn written_to_core_release_store(write: fn(&std::path::Path)) -> Vec<Vec<RecallValue>> {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let path = directory.path().join("logs/recall.sqlite3");
    core_release_store(&path);
    write(&path);
    let store = open_sqlite_store(&path, OpenMode::ReadOnly).expect("the store opens");
    query_store(
        &store,
        "SELECT (SELECT COUNT(*) FROM recall_heartbeats), (SELECT command FROM invocations), \
         (SELECT COUNT(*) FROM pragma_table_xinfo('recall_heartbeats') \
          WHERE name IN ('command', 'subcommand'))",
        None,
    )
    .expect("the query is valid")
    .rows
}

/// The order a lifecycle replacement sees events published with `priorities` and taken in one batch.
pub(crate) fn replacement_order(priorities: &[Priority]) -> Vec<usize> {
    use crate::writer::_helpers::materialize::Materializer;

    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut state = WriterState::new(envelope("ordered"));
    for (index, priority) in priorities.iter().enumerate() {
        state.push_pending(
            Box::new(RecordingReplacement {
                index,
                seen: Arc::clone(&seen),
            }),
            *priority,
        );
    }
    let batch = state.take_batch(usize::MAX);
    drop(Materializer::new(envelope("ordered"), AuditEvidence::WarnAndError).build(batch, None));
    std::mem::take(&mut *seen.lock().expect("the record is not poisoned"))
}

/// The informational notices of a session whose store opens as `failures` say after creating its database, and whether setup was refused for permission.
pub(crate) fn creation_notices(
    failures: Vec<Option<crate::store::errors::StoreErrorKind>>,
) -> (Vec<String>, bool) {
    let session = start_recall_session(SessionConfig {
        store: SessionStore::Hosted(Arc::new(CreatingStore {
            failures,
            ..CreatingStore::default()
        })),
        ..config(PathBuf::from("unused"), "creating")
    });
    let setup = session.wait_for_setup(Duration::from_secs(10));
    drop(session.close());
    (
        session.information(),
        matches!(setup, Some(SetupOutcome::PermissionDenied { .. })),
    )
}

/// One `audit_completed` event of `outcome` carrying one evidence row.
fn audit_event(index: usize, outcome: &str) -> serde_json::Map<String, serde_json::Value> {
    let event: serde_json::Value = serde_json::json!({
        "event_id": format!("audit-{index}"),
        "event_type": "audit_completed",
        "schema_version": 2,
        "producer": "sqlbuild",
        "producer_version": "1",
        "occurred_at": "2026-10-19T00:00:01.000000Z",
        "invocation_id": "evidence",
        "invocation_sequence": index,
        "resource_id": format!("audit:orders_{outcome}"),
        "payload": {
            "audit_name": format!("orders_{outcome}"),
            "outcome": outcome,
            "evidence_count": 1,
            "evidence_truncated": false,
            "evidence": [{"order_id": index}],
        },
    });
    serde_json::from_value(event).expect("the audit event is an object")
}

/// Per audit outcome: whether the stored event still holds its evidence, its `sample_digest`, and the artifact text that digest names, after a session keeping `audit_evidence`.
pub(crate) fn stored_evidence(audit_evidence: AuditEvidence) -> Vec<Vec<RecallValue>> {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let path = directory.path().join("recall.sqlite3");
    let session = start_recall_session(SessionConfig {
        audit_evidence,
        ..config(path.clone(), "evidence")
    });
    for (index, outcome) in ["warn", "error"].iter().enumerate() {
        session.publish_event_object(audit_event(index, outcome));
    }
    drop(session.close());
    let store = open_sqlite_store(&path, OpenMode::ReadOnly).expect("the store opens");
    query_store(
        &store,
        "SELECT r.outcome, json_extract(e.payload, '$.evidence') IS NOT NULL, r.sample_digest, \
         r.evidence_count, a.kind, a.text \
         FROM audit_results AS r JOIN recall_events AS e USING (event_id) \
         LEFT JOIN artifacts AS a ON a.digest = 'sha256:' || r.sample_digest \
         ORDER BY r.outcome",
        None,
    )
    .expect("the query is valid")
    .rows
}

/// Content rows, reference rows and conflicts after two statements ran the same SQL and the second failed.
pub(crate) fn failed_statement_sql() -> Vec<Vec<RecallValue>> {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let path = directory.path().join("recall.sqlite3");
    let session = start_recall_session(config(path.clone(), "failed"));
    session.publish_sql("select 1", Some("s1"), Some("orders"));
    session.publish_sql("select 1", Some("s2"), Some("orders"));
    session.publish_failed_sql("select 1", Some("s2"), Some("orders"));
    session.publish_failed_sql("select 2", Some("s3"), None);
    drop(session.close());
    let store = open_sqlite_store(&path, OpenMode::ReadOnly).expect("the store opens");
    query_store(
        &store,
        "SELECT (SELECT COUNT(*) FROM recall_artifacts), \
         (SELECT COUNT(*) FROM recall_artifact_refs), (SELECT COUNT(*) FROM recall_conflicts), \
         (SELECT group_concat(statement_id, ',' ORDER BY statement_id) FROM artifacts)",
        None,
    )
    .expect("the query is valid")
    .rows
}

pub(crate) fn recall_text(value: &str) -> RecallValue {
    RecallValue::Text(value.to_owned())
}

/// The digest of `[{"order_id":<index>}]` without its prefix.
fn evidence_digest(index: u8) -> RecallValue {
    let content: String = format!("[{{\"order_id\":{index}}}]");
    recall_text(&crate::records::main::sha256_hex::sha256_hex(
        content.as_bytes(),
    ))
}

/// The [`stored_evidence`] row of an audit whose one evidence row was kept as an artifact.
pub(crate) fn kept_evidence(outcome: &str, index: u8) -> Vec<RecallValue> {
    vec![
        recall_text(outcome),
        RecallValue::Integer(0),
        evidence_digest(index),
        RecallValue::Integer(1),
        recall_text("audit_evidence"),
        recall_text(&format!("[{{\"order_id\":{index}}}]")),
    ]
}

/// The [`stored_evidence`] row of an audit whose evidence was not kept.
pub(crate) fn left_out_evidence(outcome: &str) -> Vec<RecallValue> {
    vec![
        recall_text(outcome),
        RecallValue::Integer(0),
        RecallValue::Null,
        RecallValue::Integer(1),
        RecallValue::Null,
        RecallValue::Null,
    ]
}

/// Seconds `close` took, whether delivery reported complete, and how long after close began the final rows' deadline fell.
pub(crate) fn close_with_slow_final_rows(shutdown: Duration, delay: Duration) -> (f64, bool, f64) {
    let store = SlowFinalRowsStore {
        delay,
        final_give_up: Arc::default(),
    };
    let session = start_recall_session(SessionConfig {
        store: SessionStore::Hosted(Arc::new(store.clone())),
        shutdown_timeout: shutdown,
        maintenance: false,
        ..config(PathBuf::from("unused"), "slow-final-rows")
    });
    session.append_output(OutputStream::Stdout, b"printed\n");
    let started = std::time::Instant::now();
    let summary = session.close();
    let elapsed = started.elapsed().as_secs_f64();
    let give_up = store
        .final_give_up
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .expect("the final rows reached the store");
    (
        elapsed,
        summary.flush_complete,
        give_up.saturating_duration_since(started).as_secs_f64(),
    )
}
