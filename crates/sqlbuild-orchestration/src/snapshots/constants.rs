//! Snapshot retention keeps recent publications per location and target, collected in small batches.

pub(crate) const DEFAULT_SNAPSHOT_RETENTION_COUNT: u16 = 10;
pub(crate) const SNAPSHOT_RETENTION_BATCH: usize = 10;
pub(crate) const CONTENT_HASH_CHUNK: usize = 500;

/// `{node_key: {column: {"expression_range": [start, end]}}}`; readers skip unknown column keys.
pub(crate) const COLUMNS_DOCUMENT: &str = "columns";
/// Snapshot document kinds: per-snapshot metadata that never changes a node hash or node change.
pub(crate) const SNAPSHOT_DOCUMENT_KINDS: [&str; 7] = [
    "jobs",
    "groups",
    "triggers",
    "roles",
    "lineage",
    COLUMNS_DOCUMENT,
    "providers",
];

/// Beyond the newest generations, any retained reference keeps a snapshot.
pub(crate) const EXPIRED_SNAPSHOTS: &str = "SELECT s.snapshot_id FROM sqb_snapshots s \
    WHERE (SELECT COUNT(*) FROM sqb_snapshots newer WHERE newer.location_id=s.location_id \
        AND newer.target=s.target AND newer.generation>s.generation)>=$1 \
    AND NOT EXISTS (SELECT 1 FROM sqb_current_snapshots c WHERE c.snapshot_id=s.snapshot_id) \
    AND NOT EXISTS (SELECT 1 FROM sqb_runs r WHERE r.snapshot_id=s.snapshot_id) \
    AND NOT EXISTS (SELECT 1 FROM sqb_jobs j WHERE j.snapshot_id=s.snapshot_id) \
    AND NOT EXISTS (SELECT 1 FROM sqb_triggers t WHERE t.snapshot_id=s.snapshot_id \
        OR t.removed_in_snapshot_id=s.snapshot_id) \
    AND NOT EXISTS (SELECT 1 FROM sqb_location_work w WHERE w.snapshot_id=s.snapshot_id) \
    ORDER BY s.published_at, s.snapshot_id LIMIT $2";

/// Content-addressed bodies of the expiring snapshots, removed once nothing else references them.
pub(crate) const CONTENT: [((&str, &str), &str, &str); 3] = [
    (
        ("sqb_snapshot_nodes", "node_hash"),
        "sqb_node_definitions",
        "",
    ),
    (
        ("sqb_snapshot_sources", "text_hash"),
        "sqb_source_texts",
        " AND NOT EXISTS (SELECT 1 FROM sqb_warehouse_fingerprints f \
         WHERE f.definition_text_hash=sqb_source_texts.text_hash)",
    ),
    (
        ("sqb_snapshot_documents", "document_hash"),
        "sqb_documents",
        "",
    ),
];
