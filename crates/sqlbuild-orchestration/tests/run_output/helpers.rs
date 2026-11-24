//! A claimed run, an output store in a temporary folder, a person who may read evidence, and batches.

use std::collections::BTreeMap;
use std::path::Path;

use futures_util::StreamExt;
use uuid::Uuid;

use crate::authorization::main::people::bootstrap_user::bootstrap_user;
use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::run_output::_helpers::chunks::level_of;
use crate::run_output::main::open_output_store::open_output_store;
use crate::run_output::models::{
    OutputBatch, OutputLine, OutputPageQuery, OutputRow, OutputSegment, OutputSettings,
    OutputStore, OutputStream,
};
use crate::run_output::tests::test_types::PageStart;
use crate::runs::main::execution::claim_next_run::claim_next_run;
use crate::runs::models::RunClaim;
use crate::runs::tests::helpers::{queue, request, snapshot};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::ControlStore;

/// A migrated store's location and a claimed run that selects `orders`.
pub(crate) async fn claimed_run(store: &ControlStore) -> (Uuid, RunClaim) {
    migrate_store(store).await.expect("migrate");
    let location = create_location(store, "orders", "test-admin")
        .await
        .expect("location");
    let published = publish_snapshot(store, &snapshot(location))
        .await
        .expect("publish");
    queue(
        store,
        &request((location, published.snapshot_id), 61, 0, &["orders"]),
    )
    .await;
    let lease = acquire_lease(store, Uuid::now_v7())
        .await
        .expect("elect")
        .expect("leader");
    let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
        .await
        .expect("claim")
        .expect("claimable");
    (location, claim)
}

/// An output store in `folder`, keeping at most `max_bytes_per_run` of a run.
pub(crate) fn output_store(
    store: &ControlStore,
    folder: &Path,
    max_bytes_per_run: Option<u64>,
) -> OutputStore {
    open_output_store(
        &OutputSettings {
            store: Some(format!("file://{}", folder.display())),
            max_bytes_per_run,
        },
        store,
    )
    .expect("output store")
}

/// An administrator who may read every run's evidence, and the audit context they read with.
pub(crate) async fn reader(store: &ControlStore) -> (AuditContext, Uuid) {
    let context = AuditContext {
        actor: "test-operator".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    let admin = bootstrap_user(store, &context, "orders-admin")
        .await
        .expect("administrator");
    (context, admin)
}

pub(crate) fn segment(node_key: Option<&str>, stream: OutputStream, text: &str) -> OutputSegment {
    OutputSegment {
        node_key: node_key.map(str::to_owned),
        stream,
        text: text.into(),
        read_at: read_at(0),
        dropped_bytes: 0,
    }
}

/// A read time `millis` milliseconds after a fixed moment.
pub(crate) fn read_at(millis: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_millis(1_795_000_000_000 + millis).expect("read time")
}

pub(crate) fn batch(number: u64, segments: Vec<OutputSegment>) -> OutputBatch {
    OutputBatch {
        batch: number,
        segments,
    }
}

/// How many objects the output store holds.
pub(crate) async fn object_count(output: &OutputStore) -> usize {
    output.objects.list(None).count().await
}

/// A stored line as a page returns it, its level read the way the server reads it.
pub(crate) fn line(number: i64, node_key: &str, stream: OutputStream, text: &str) -> OutputLine {
    OutputLine {
        number,
        node_key: node_key.into(),
        stream,
        text: text.into(),
        at: chrono::DateTime::UNIX_EPOCH,
        level: level_of(text),
        omitted_before: 0,
    }
}

/// `rows` as JSON with every line's arrival time left out, to compare with lines whose time a test cannot know.
pub(crate) fn timeless(rows: &[OutputRow]) -> serde_json::Value {
    let mut value: serde_json::Value = serde_json::to_value(rows).expect("rows");
    for row in value.as_array_mut().expect("rows") {
        row["line"]["at"] = serde_json::Value::Null;
        row["sample"]["at"] = serde_json::Value::Null;
    }
    value
}

/// A page row of one line.
pub(crate) fn line_row(number: i64, node_key: &str, stream: OutputStream, text: &str) -> OutputRow {
    OutputRow::Line {
        line: line(number, node_key, stream, text),
    }
}

/// A page row folding `count` similar lines from `sample`'s.
pub(crate) fn similar_row(count: i64, sample: OutputLine) -> OutputRow {
    OutputRow::Similar {
        from: sample.number,
        count,
        sample,
    }
}

/// A page query for one node's view, or the run's, from `start`.
pub(crate) fn page_query(
    (node, stream): (Option<&str>, Option<OutputStream>),
    start: PageStart,
    (limit, collapse, around): (u32, bool, Option<i64>),
) -> OutputPageQuery {
    OutputPageQuery {
        node: node.map(str::to_owned),
        stream,
        from: start.from,
        before: start.before,
        tail: start.tail,
        limit: Some(limit),
        collapse,
        around,
    }
}

/// Make the folder holding the run's chunks read-only, or writable again, so deleting them fails or works.
#[cfg(unix)]
pub(crate) async fn set_chunks_writable(output: &OutputStore, folder: &Path, writable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let first = output
        .objects
        .list(None)
        .next()
        .await
        .expect("a stored chunk")
        .expect("listed");
    let chunks = folder.join(first.location.as_ref()).with_file_name("");
    let mode: u32 = [0o555, 0o755][usize::from(writable)];
    std::fs::set_permissions(&chunks, std::fs::Permissions::from_mode(mode)).expect("permissions");
}

/// A case's path, with a leading `/` read as this platform's root, the temporary folder's, so it is absolute there too.
pub(crate) fn platform_path(path: &str) -> std::path::PathBuf {
    let temporary: std::path::PathBuf = std::env::temp_dir();
    let root: &std::path::Path = temporary.ancestors().last().expect("a filesystem root");
    path.strip_prefix('/')
        .map_or_else(|| std::path::PathBuf::from(path), |rest| root.join(rest))
}
