//! What a location's warehouse sync reports: its progress lines, health reports and page sizes.

use chrono::{DateTime, Utc};
use sqlbuild_core::number_text::main::format_thousands::format_thousands;
use sqlbuild_orchestration::platform::models::{
    LocationHealth, LocationHealthReport, LocationWorkClient, LocationWorkPoll,
    WarehouseSyncFailure, WarehouseSyncProgress,
};
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseHistoryKind;
use tokio::time::Instant;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::location_warning;
use crate::native_commands::constants::{
    METADATA_ERROR_CHARS, METADATA_POLL_SECONDS, STATISTICS_RELATION_SECONDS,
};
use crate::native_commands::models::{LocationMetadataSync, ReadFailure};

/// One table's line as its scan completes, e.g. `Warehouse history 3/14: audit results, 1,204 rows in 0.8 s`.
pub(crate) fn scan_line(state: &LocationMetadataSync, scans: usize) -> String {
    let namespace = if state.locations.len() > 1 {
        let location = state.location();
        match &location.database {
            Some(database) => format!(" in {database}.{}", location.schema),
            None => format!(" in {}", location.schema),
        }
    } else {
        String::new()
    };
    let read = if state.progress.scan_unchanged {
        "unchanged".to_owned()
    } else {
        rows_text(state.progress.scan_rows)
    };
    format!(
        "Warehouse history {}/{scans}: {}{namespace}, {read} in {:.1} s\n",
        state.index + 1,
        state.kind().as_str().replace('_', " "),
        state.progress.scan_started.elapsed().as_secs_f64()
    )
}

/// Where a sync has got to; a clean report of the last table's last page completes it.
pub(crate) fn sync_progress(state: &LocationMetadataSync, clean: bool) -> WarehouseSyncProgress {
    let kinds = (state.locations.len() * WarehouseHistoryKind::ALL.len()) as u64;
    let page = state.pending.as_ref();
    let rows = state.progress.sync_rows
        + state.progress.scan_rows
        + page.map_or(0, |page| page.batch.records.len() as u64);
    let scan_done = page.is_none_or(|page| page.next_source_after.is_none());
    WarehouseSyncProgress {
        position: state.index as u64 + 1,
        kinds,
        rows,
        elapsed_millis: state.progress.sync_started.elapsed().as_millis() as u64,
        completed: clean && scan_done && state.index as u64 + 1 == kinds,
        halted: state.halted,
    }
}

/// A row count with thousands separators, e.g. `31,204 rows`.
pub(crate) fn rows_text(rows: u64) -> String {
    format!(
        "{} {}",
        format_thousands(i128::from(rows)),
        if rows == 1 { "row" } else { "rows" }
    )
}

/// When the next safety sync is due once this report's page completes the sync; `None` without a timed one.
pub(crate) fn completed_sync_next_at(
    state: &LocationMetadataSync,
    failure: Option<WarehouseSyncFailure>,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let completes = sync_progress(state, failure.is_none()).completed;
    completes.then(|| safety_sync_at(state, now)).flatten()
}

/// When a timed safety sync follows from `now`, under the same rules as `completed_sync_next_at`; an interval too large to add never comes due.
pub(crate) fn safety_sync_at(
    state: &LocationMetadataSync,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    if state.safety_seconds == 0 || state.defers_safety_sync() {
        return None;
    }
    let seconds: i64 = match i64::try_from(state.safety_seconds) {
        Ok(seconds) => seconds,
        Err(_beyond_range) => return None,
    };
    now.checked_add_signed(chrono::Duration::try_seconds(seconds)?)
}

/// A page's health report: a failed read adds its cause, redacted error, page size and whether it stops the sync.
pub(crate) fn metadata_health_of(
    state: &LocationMetadataSync,
    (started, failure, read): (
        Instant,
        Option<WarehouseSyncFailure>,
        Option<(&ReadFailure, bool)>,
    ),
    now: DateTime<Utc>,
) -> LocationHealth {
    let halted: bool = read.is_some_and(|(_, halts)| halts);
    let next_sync_at = if halted {
        safety_sync_at(state, now)
    } else {
        completed_sync_next_at(state, failure, now)
    };
    let read_failure = read.and_then(|(read, _)| read.cause());
    LocationHealth::WarehouseSync {
        generation: state.generation,
        location: state.location().clone(),
        table: state.kind(),
        failure,
        records_sent: state
            .pending
            .as_ref()
            .map_or(0, |page| page.batch.records.len()) as u64,
        elapsed_millis: started.elapsed().as_millis() as u64,
        page_limit: read_failure.map(|_| page_limit(state) as u64),
        read_failure,
        error: read
            .filter(|(read, _)| read.cause().is_some())
            .map(|(read, _)| read.reason().chars().take(METADATA_ERROR_CHARS).collect()),
        progress: Some(WarehouseSyncProgress {
            halted,
            ..sync_progress(state, failure.is_none())
        }),
        next_sync_at,
    }
}

/// Send `metadata_health_of` for this page; a failed report is logged, never fatal.
pub(crate) async fn report_metadata_health(
    state: &LocationMetadataSync,
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    (started, failure, read): (
        Instant,
        Option<WarehouseSyncFailure>,
        Option<(&ReadFailure, bool)>,
    ),
) {
    let report = LocationHealthReport {
        event_id: Uuid::now_v7(),
        session_id: poll.session_id,
        target: poll.target.clone(),
        health: metadata_health_of(state, (started, failure, read), Utc::now()),
    };
    if let Err(error) = client.report_health(&report).await {
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Warehouse synchronization health report failed: {error}\n"),
        )]));
    }
}

/// Operational cost is elapsed IO time, not an estimate of warehouse billing.
pub(crate) async fn metadata_health(
    state: &LocationMetadataSync,
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    (started, failure): (Instant, Option<WarehouseSyncFailure>),
) {
    report_metadata_health(state, (client, poll), (started, failure, None)).await;
}

/// How long one page may take: statistics scale with their relations, other pages get a fixed allowance.
pub(crate) fn read_timeout_seconds(state: &LocationMetadataSync) -> u64 {
    if state.kind() == WarehouseHistoryKind::Statistics {
        METADATA_POLL_SECONDS + STATISTICS_RELATION_SECONDS * state.statistics_limit as u64
    } else {
        state.page_seconds
    }
}

/// The page size a read asked for: statistics relations, or rows of any other table.
pub(crate) fn page_limit(state: &LocationMetadataSync) -> usize {
    if state.kind() == WarehouseHistoryKind::Statistics {
        state.statistics_limit
    } else {
        state.limit
    }
}

/// Say a stopped sync resumes; `one_attempt` when one more failure stops it again.
pub(crate) fn resumed_line(one_attempt: bool) {
    drop(write_output(&[(
        Stream::Stderr,
        "Retrying the stopped warehouse history synchronization.\n",
    )]));
    location_warning(
        "warehouse sync retrying",
        if one_attempt {
            "one attempt; another failed read stops it again"
        } else {
            "on request; failed reads back off again before they stop it"
        }
        .to_owned(),
    );
}

/// What failed, on which table and page size, and why.
pub(crate) fn read_failure_line(state: &LocationMetadataSync, failure: &ReadFailure) -> String {
    let unit = if state.kind() == WarehouseHistoryKind::Statistics {
        "relation"
    } else {
        "row"
    };
    format!(
        "Warehouse history read failed for {} ({}-{unit} page): {}.",
        state.kind().as_str().replace('_', " "),
        page_limit(state),
        failure.reason(),
    )
}
