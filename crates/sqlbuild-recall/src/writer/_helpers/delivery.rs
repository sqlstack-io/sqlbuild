//! The delivery thread: table setup, batched writes with retries, heartbeats, the final delivery rows and maintenance. It alone opens and uses the store connection.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use crate::records::main::delivery_row::delivery_row;
use crate::records::main::format_timestamp::format_timestamp;
use crate::records::main::heartbeat_row::heartbeat_row;
use crate::records::models::RecallRow;
use crate::schema::models::Table;
use crate::store::constants::{BUSY_TIMEOUT_MS, MAINTENANCE_BUDGET_MS};
use crate::store::errors::{StoreError, StoreErrorKind};
use crate::store::main::checkpoint_sqlite_store::checkpoint_sqlite_store;
use crate::store::main::ensure_sqlite_schema::ensure_sqlite_schema;
use crate::store::main::insert_rows::insert_rows;
use crate::store::main::maintain_sqlite_store::maintain_sqlite_store;
use crate::store::main::observe_store_file::observe_store_file;
use crate::store::main::open_sqlite_store::open_sqlite_store;
use crate::store::main::quarantine_store::quarantine_store;
use crate::store::models::{
    InsertOutcome, MaintenanceRequest, OpenMode, QuarantineOutcome, SqliteStore, StoreObservation,
};
use crate::store::types::{RecallStore, RecallStoreOpener};
use crate::writer::_helpers::materialize::{Built, Materializer};
use crate::writer::_helpers::queue::{Queued, WriterState};
use crate::writer::constants::{
    BATCH_ROWS, BATCH_WINDOW_MS, CLOCK_CHECK_MS, EARLY_BATCH_BYTES, FINAL_ROWS_BUDGET_MS,
    HEARTBEAT_STALE_INTERVALS, MAX_RETRY_BACKOFF_MS, MINIMUM_MAINTENANCE_MS, OUTPUT_COALESCE_MS,
    RUNNING_RETRY_LIMIT_MS, SETUP_RETRY_MS,
};
use crate::writer::models::{DeliveryStream, SessionConfig, SessionStore, SetupOutcome};
use crate::writer::types::OutputReplacement;

/// State shared by producers, the session handle and the delivery thread.
#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) state: Mutex<WriterState>,
    pub(crate) wake: Condvar,
    pub(crate) setup: Mutex<Option<SetupOutcome>>,
    pub(crate) setup_done: Condvar,
    /// The replacement sink output passes through, once one is configured.
    pub(crate) output_replacement: OnceLock<Arc<dyn OutputReplacement>>,
}

pub(crate) fn locked(mutex: &Mutex<WriterState>) -> MutexGuard<'_, WriterState> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn publish_setup(shared: &Shared, outcome: SetupOutcome) {
    let mut setup = shared
        .setup
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *setup = Some(outcome);
    shared.setup_done.notify_all();
}

/// When busy retries stop: the close deadline once closing, else the running retry limit.
fn retry_limit(shared: &Shared, started: Instant) -> Instant {
    locked(&shared.state)
        .deadline
        .unwrap_or_else(|| started + Duration::from_millis(RUNNING_RETRY_LIMIT_MS))
}

/// Each SQLite busy wait ends by `give_up`: the shutdown deadline, or 500 ms on for the final rows.
fn bound_busy_wait(store: &SqliteStore, give_up: Instant) {
    let remaining: Duration = give_up
        .saturating_duration_since(Instant::now())
        .min(Duration::from_millis(BUSY_TIMEOUT_MS));
    drop(store.connection.busy_timeout(remaining));
}

/// The native SQLite store as the delivery thread writes it.
struct NativeSqlite(SqliteStore);

impl RecallStore for NativeSqlite {
    fn insert(&self, rows: &[RecallRow], give_up: Instant) -> Result<InsertOutcome, StoreError> {
        bound_busy_wait(&self.0, give_up);
        let now = format_timestamp(SystemTime::now());
        insert_rows(&self.0, rows, &now)
    }

    fn maintain(&self, request: &MaintenanceRequest) {
        bound_busy_wait(&self.0, Instant::now() + request.budget);
        drop(maintain_sqlite_store(&self.0, request));
    }
}

fn open_hosted(
    shared: &Shared,
    config: &SessionConfig,
    opener: &dyn RecallStoreOpener,
) -> Result<Box<dyn RecallStore>, StoreError> {
    let started = Instant::now();
    loop {
        match opener.open(&config.envelope, retry_limit(shared, started)) {
            Err(error)
                if error.kind == StoreErrorKind::Busy
                    && Instant::now() < retry_limit(shared, started) =>
            {
                std::thread::sleep(Duration::from_millis(SETUP_RETRY_MS));
            }
            other => return other,
        }
    }
}

fn open_store(shared: &Shared, config: &SessionConfig) -> Result<Box<dyn RecallStore>, StoreError> {
    match &config.store {
        SessionStore::Sqlite(path) => open_and_setup(shared, config, path).map(|store| {
            drop(checkpoint_sqlite_store(&store));
            Box::new(NativeSqlite(store)) as Box<dyn RecallStore>
        }),
        SessionStore::Hosted(opener) => {
            let opened = open_hosted(shared, config, opener.as_ref());
            let information: Vec<String> = opener.take_information();
            locked(&shared.state).information.extend(information);
            opened
        }
    }
}

fn open_and_setup(
    shared: &Shared,
    config: &SessionConfig,
    store_path: &std::path::Path,
) -> Result<SqliteStore, StoreError> {
    let started = Instant::now();
    let mut quarantined: bool = false;
    loop {
        let observed: Option<StoreObservation> = observe_store_file(store_path).unwrap_or(None);
        let attempt = open_sqlite_store(store_path, OpenMode::ReadWrite).and_then(|store| {
            bound_busy_wait(&store, retry_limit(shared, started));
            let now = format_timestamp(SystemTime::now());
            ensure_sqlite_schema(&store, &config.envelope, &now).map(|()| store)
        });
        match attempt {
            Err(error)
                if error.kind == StoreErrorKind::Busy
                    && Instant::now() < retry_limit(shared, started) =>
            {
                std::thread::sleep(Duration::from_millis(SETUP_RETRY_MS));
            }
            Err(error) if error.kind == StoreErrorKind::Corrupt && !quarantined => {
                quarantined = true;
                let Some(observed) = observed else {
                    return Err(error);
                };
                let outcome = quarantine_store(
                    store_path,
                    &observed,
                    (retry_limit(shared, started), SystemTime::now()),
                )?;
                match outcome {
                    QuarantineOutcome::Moved(moved) => {
                        locked(&shared.state).notices.push(format!(
                            "the Recall store {} was not a valid database; it was moved to {} \
                             and a fresh store was started",
                            store_path.display(),
                            moved.display()
                        ));
                    }
                    QuarantineOutcome::Kept => {}
                    QuarantineOutcome::LockBusy => return Err(error),
                }
            }
            other => return other,
        }
    }
}

/// Write one batch, retrying a busy store with backoff until `give_up`.
fn write_batch(
    store: &dyn RecallStore,
    rows: &[RecallRow],
    give_up: Instant,
) -> Result<InsertOutcome, StoreError> {
    let mut backoff = Duration::from_millis(10);
    loop {
        match store.insert(rows, give_up) {
            Err(error) if error.kind == StoreErrorKind::Busy && Instant::now() < give_up => {
                std::thread::sleep(backoff.min(give_up.saturating_duration_since(Instant::now())));
                backoff = (backoff * 2).min(Duration::from_millis(MAX_RETRY_BACKOFF_MS));
            }
            other => return other,
        }
    }
}

/// A heartbeat at `now`; the `first` names the command before the context row may be written.
fn heartbeat(config: &SessionConfig, now: SystemTime, first: bool) -> RecallRow {
    let stale = now + config.heartbeat_interval * HEARTBEAT_STALE_INTERVALS;
    let context = &config.context;
    heartbeat_row(
        &config.envelope,
        (&format_timestamp(now), &format_timestamp(stale)),
        (context.host_token.as_deref(), context.pid),
        if first {
            (context.command.as_deref(), context.subcommand.as_deref())
        } else {
            (None, None)
        },
    )
}

/// The stream whose records are stored in `table`; `None` for the store's own bookkeeping.
pub(crate) fn record_stream(table: Table) -> Option<DeliveryStream> {
    match table {
        Table::Heartbeats | Table::Delivery | Table::Meta | Table::Conflicts => None,
        other => Some(stream_of(other)),
    }
}

pub(crate) fn stream_of(table: Table) -> DeliveryStream {
    match table {
        Table::Output => DeliveryStream::Output,
        Table::Events => DeliveryStream::Events,
        Table::Diagnostics => DeliveryStream::Diagnostics,
        Table::Artifacts | Table::ArtifactRefs => DeliveryStream::Artifacts,
        _ => DeliveryStream::Invocation,
    }
}

/// How many rows a batch takes and how long it waits to fill.
#[derive(Clone, Copy)]
struct Batching {
    rows: usize,
    window: Duration,
}

fn batching(config: &SessionConfig) -> Batching {
    match &config.store {
        SessionStore::Sqlite(_) => Batching {
            rows: BATCH_ROWS,
            window: Duration::from_millis(BATCH_WINDOW_MS),
        },
        SessionStore::Hosted(opener) => Batching {
            rows: opener.batch_rows(),
            window: opener.batch_window(),
        },
    }
}

/// Wait until a batch is due: its row or byte limit, the batch window, closing, or a timed duty.
fn next_batch(
    shared: &Shared,
    next_heartbeat: SystemTime,
    batching: Batching,
) -> (Vec<Queued>, bool) {
    let mut state = locked(&shared.state);
    let mut first_seen: Option<Instant> = None;
    loop {
        let now = Instant::now();
        if state
            .output_due(Duration::from_millis(OUTPUT_COALESCE_MS))
            .is_some_and(|due| due <= now)
            || (state.closing && state.has_pending_output())
        {
            state.flush_output();
        }
        if state.ready() > 0 && first_seen.is_none() {
            first_seen = Some(now);
        }
        let window_over =
            first_seen.is_some_and(|seen| now.duration_since(seen) >= batching.window);
        if state.ready() >= batching.rows
            || state.ready_bytes() >= EARLY_BATCH_BYTES
            || (state.ready() > 0 && (window_over || state.closing))
        {
            return (state.take_batch(batching.rows), state.closing);
        }
        if state.closing && state.queued() == 0 && !state.has_pending_output() {
            return (Vec::new(), true);
        }
        let until_heartbeat: Duration = next_heartbeat
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO);
        if until_heartbeat.is_zero() {
            return (state.take_batch(batching.rows), state.closing);
        }
        let mut wait_until = now + until_heartbeat.min(Duration::from_millis(CLOCK_CHECK_MS));
        if let Some(due) = state.output_due(Duration::from_millis(OUTPUT_COALESCE_MS)) {
            wait_until = wait_until.min(due);
        }
        if let Some(seen) = first_seen {
            wait_until = wait_until.min(seen + batching.window);
        }
        let timeout = wait_until
            .saturating_duration_since(now)
            .max(Duration::from_millis(1));
        state = shared
            .wake
            .wait_timeout(state, timeout)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0;
    }
}

fn degrade_and_drain(shared: &Shared, reason: String) {
    let mut state = locked(&shared.state);
    state.degraded.get_or_insert(reason);
    state.rejecting = true;
    state.drop_all();
    state.finished = true;
    shared.wake.notify_all();
}

fn final_rows(config: &SessionConfig, state: &WriterState) -> Vec<RecallRow> {
    let now = format_timestamp(SystemTime::now());
    let complete = state.degraded.is_none();
    DeliveryStream::ALL
        .iter()
        .map(|stream| {
            let counts = state.counts[stream.index()];
            let flushed = complete && counts.dropped == 0 && counts.failed == 0;
            delivery_row(&config.envelope, stream.as_str(), (&counts, flushed), &now)
        })
        .collect()
}

/// The delivery thread body.
pub(crate) fn deliver(shared: &Arc<Shared>, config: &SessionConfig) {
    let mut store: Box<dyn RecallStore> = match open_store(shared, config) {
        Ok(store) => {
            publish_setup(shared, SetupOutcome::Ready);
            store
        }
        Err(error) => {
            let outcome = if error.kind == StoreErrorKind::Permission {
                SetupOutcome::PermissionDenied {
                    reason: error.message.clone(),
                }
            } else {
                SetupOutcome::Failed {
                    reason: error.message.clone(),
                }
            };
            publish_setup(shared, outcome);
            degrade_and_drain(shared, error.message);
            return;
        }
    };
    let mut next_heartbeat: SystemTime = SystemTime::now();
    let mut first_heartbeat: bool = true;
    let mut materializer = Materializer::new(config.envelope.clone(), config.audit_evidence);
    let batching: Batching = batching(config);
    loop {
        let (batch, closing) = next_batch(shared, next_heartbeat, batching);
        let built: Built =
            materializer.build(batch, shared.output_replacement.get().map(Arc::as_ref));
        if !built.settled.is_empty()
            || !built.unbuildable.is_empty()
            || !built.filtered.is_empty()
            || built.extra_events > 0
            || built.extra_artifacts > 0
        {
            let mut state = locked(&shared.state);
            state.counts_mut(DeliveryStream::Events).accepted += built.extra_events;
            state.counts_mut(DeliveryStream::Artifacts).accepted += built.extra_artifacts;
            state.account(&built.settled, Ok(&InsertOutcome::default()));
            state.account_unbuildable(&built.unbuildable);
            state.account_filtered(&built.filtered);
        }
        let mut rows: Vec<RecallRow> = built.rows;
        let now: SystemTime = SystemTime::now();
        if now >= next_heartbeat && !closing {
            rows.push(heartbeat(config, now, first_heartbeat));
            first_heartbeat = false;
            next_heartbeat = now + config.heartbeat_interval;
        }
        if !rows.is_empty() {
            let give_up = {
                let state = locked(&shared.state);
                state.deadline.unwrap_or_else(|| {
                    Instant::now() + Duration::from_millis(RUNNING_RETRY_LIMIT_MS)
                })
            };
            let written = write_batch(store.as_ref(), &rows, give_up);
            let mut state = locked(&shared.state);
            match &written {
                Ok(outcome) => state.account(&built.written, Ok(outcome)),
                Err(error) => state.account(&built.written, Err(&error.message)),
            }
        }
        let mut state = locked(&shared.state);
        let expired = state
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline);
        if expired {
            state.flush_output();
            state.drop_all();
        }
        if (closing || expired) && state.queued() == 0 && !state.has_pending_output() {
            state.rejecting = true;
            let rows = final_rows(config, &state);
            drop(state);
            let give_up = retry_limit(shared, Instant::now())
                .max(Instant::now() + Duration::from_millis(FINAL_ROWS_BUDGET_MS));
            if let Err(error) = write_batch(store.as_ref(), &rows, give_up) {
                locked(&shared.state).degraded.get_or_insert(error.message);
            }
            break;
        }
    }
    let remaining: Duration = retry_limit(shared, Instant::now())
        .saturating_duration_since(Instant::now())
        .min(Duration::from_millis(MAINTENANCE_BUDGET_MS));
    if config.maintenance && remaining >= Duration::from_millis(MINIMUM_MAINTENANCE_MS) {
        let request = MaintenanceRequest {
            now: SystemTime::now(),
            retention_days: config.retention_days,
            owner: config.maintenance_owner.clone(),
            budget: remaining,
            envelope: config.envelope.clone(),
        };
        store.maintain(&request);
    }
    let notices: Vec<String> = store.take_notices();
    store.close();
    drop(store);
    let mut state = locked(&shared.state);
    state.notices.extend(notices);
    state.finished = true;
    shared.wake.notify_all();
}
